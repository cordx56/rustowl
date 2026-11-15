//! What a snapshot cannot say: how many documents arrive, what the analysis
//! computes, and whether the cache is used.

use archery::{ArcK, SharedPointer};
use rustowl::models::{Function, MirDecl, Range, Workspace};
use rustowl::toolchain;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

const REFERENCE_FUNCTION_COUNT: usize = 74;

/// The perf fixture's `[lib]` and `[[bin]]`: a check compiles no example or bench.
const REFERENCE_CRATES: usize = 2;

/// What the reference output records for one function.
#[derive(Serialize)]
struct ReferenceFunction {
    fn_id: u32,
    name: String,
    decls: Vec<MirDecl>,
}

/// Everything rustowlc printed, keyed crate -> file, before any collapsing.
type RawWorkspace = BTreeMap<String, BTreeMap<String, Vec<SharedPointer<Function, ArcK>>>>;

/// Canonical form of a [`RawWorkspace`]: crate -> file -> functions.
type ReferenceWorkspace = BTreeMap<String, BTreeMap<String, Vec<ReferenceFunction>>>;

/// Append every function of one document to its (crate, file) bucket.
fn accumulate(into: &mut RawWorkspace, workspace: Workspace) {
    let Workspace(crates) = workspace;
    for (crate_name, krate) in crates.iter() {
        let per_file = into.entry(crate_name.to_string()).or_default();
        for (file, payload) in krate.0.iter() {
            let bucket = per_file.entry(file.to_string()).or_default();
            for item in payload.items.iter() {
                bucket.push(SharedPointer::clone(item));
            }
        }
    }
}

/// Strip the fixture root, so the snapshot carries no absolute path.
fn relative_file_name(file: &str, fixture_root: &Path) -> String {
    Path::new(file)
        .strip_prefix(fixture_root)
        .unwrap_or_else(|_| {
            panic!("rustowlc reported {file}, which is outside the fixture {fixture_root:?}")
        })
        .to_string_lossy()
        .replace('\\', "/")
}

fn decl_local_id(decl: &MirDecl) -> u32 {
    match decl {
        MirDecl::User { local, .. } | MirDecl::Other { local, .. } => local.id,
    }
}

/// The lists are shared, so a sorted one is built rather than sorted in place.
fn sorted_ranges(ranges: &[Range]) -> Arc<[Range]> {
    let mut ranges = ranges.to_vec();
    ranges.sort_by_key(|range| (range.from().0, range.until().0));
    Arc::from(ranges)
}

/// Sort the parts of a decl whose order is not deterministic in production.
fn normalize_decl(decl: &mut MirDecl) {
    let (MirDecl::User {
        lives,
        shared_borrow,
        mutable_borrow,
        drop_range,
        definitely_live_at,
        maybe_init_at,
        deficit_at,
        storage_range,
        ..
    }
    | MirDecl::Other {
        lives,
        shared_borrow,
        mutable_borrow,
        drop_range,
        definitely_live_at,
        maybe_init_at,
        deficit_at,
        storage_range,
        ..
    }) = decl;

    *lives = sorted_ranges(lives);
    *shared_borrow = sorted_ranges(shared_borrow);
    *mutable_borrow = sorted_ranges(mutable_borrow);
    *drop_range = sorted_ranges(drop_range);
    *definitely_live_at = sorted_ranges(definitely_live_at);
    *maybe_init_at = sorted_ranges(maybe_init_at);
    *deficit_at = sorted_ranges(deficit_at);
    *storage_range = sorted_ranges(storage_range);
}

/// Canonical form of one delivered function: decls by local id, and the ranges
/// inside each decl by source order.
fn normalize_function(function: &mut Function) {
    function.decls.sort_by_key(decl_local_id);
    for decl in &mut function.decls {
        normalize_decl(decl);
    }
}

/// Serialize a function for the duplicate-content check.
fn canonical_json(function: &Function) -> Result<String, String> {
    serde_json::to_string(function).map_err(|error| error.to_string())
}

/// Collapse repeated deliveries of one function, refusing to do so silently.
fn normalize_items(mut items: Vec<SharedPointer<Function, ArcK>>) -> Vec<ReferenceFunction> {
    items.sort_by_key(|item| item.fn_id);
    for item in &mut items {
        // The harness owns every body, so this normalises in place.
        normalize_function(SharedPointer::make_mut(item));
    }

    let mut deduped: Vec<SharedPointer<Function, ArcK>> = Vec::with_capacity(items.len());
    for item in items {
        if let Some(previous) = deduped.last()
            && previous.fn_id == item.fn_id
        {
            assert!(
                canonical_json(previous) == canonical_json(&item),
                "fn_id {} ({}) was reported twice with different contents; collapsing \
                 them would hide the difference",
                item.fn_id,
                item.name
            );
        } else {
            deduped.push(item);
        }
    }

    deduped
        .into_iter()
        .map(|function| {
            let function = SharedPointer::try_unwrap(function)
                .expect("the harness must own every delivered body");
            ReferenceFunction {
                fn_id: function.fn_id,
                name: function.name.to_string(),
                decls: function.decls,
            }
        })
        .collect()
}

fn normalize_workspace(raw: RawWorkspace, fixture_root: &Path) -> ReferenceWorkspace {
    raw.into_iter()
        .map(|(crate_name, files)| {
            let per_file = files
                .into_iter()
                .map(|(file, items)| {
                    (
                        relative_file_name(&file, fixture_root),
                        normalize_items(items),
                    )
                })
                .collect();
            (crate_name, per_file)
        })
        .collect()
}

async fn workspace_packages(cargo: &str, fixture: &Path) -> Vec<String> {
    cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo)
        .current_dir(fixture)
        .no_deps()
        .exec()
        .expect("cargo metadata must run over the perf fixture")
        .packages
        .iter()
        .map(|package| package.name.to_string())
        .collect()
}

/// Fail loudly instead of letting the sysroot lookup kill the whole test binary.
fn require_toolchain_sysroot() {
    let runtime = &*toolchain::FALLBACK_RUNTIME_DIR;
    let sysroot = toolchain::sysroot_from_runtime(runtime);
    assert!(
        runtime.is_dir() && sysroot.is_dir(),
        "no rustowl toolchain sysroot at {sysroot:?}. `test_workspace_reference` compiles \
         the perf fixture against it, and running the test without one makes \
         toolchain::get_runtime_dir() download a toolchain and, on failure, \
         std::process::exit(1), killing every test in this file with a bare exit \
         code. Populate it first, e.g. via ./scripts/toolchain."
    );
}

/// A cargo command wired the way `rustowl check` wires it: `rustowlc` as the
/// compiler, the wrapper's sysroot in the rustflags, and nothing ambient.
///
/// RUSTC is overridden because `setup_cargo_command` resolves `rustowlc` next to the
/// test harness in `target/<profile>/deps/`, where no such binary exists.
async fn rustowl_cargo_command(rustowlc: &str, target_dir: &Path) -> tokio::process::Command {
    let mut command = toolchain::setup_cargo_command().await;
    let sysroot = toolchain::get_sysroot().await;
    command
        .env("RUSTC", rustowlc)
        .env("RUSTC_WORKSPACE_WRAPPER", rustowlc)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTOWL_CACHE_DIR", target_dir.join("cache"))
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            format!("--sysroot={}", sysroot.display()),
        )
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_BUILD_RUSTFLAGS")
        .env_remove("CARGO_BUILD_TARGET")
        .env_remove("RUSTOWL_OPEN_FILES")
        .env_remove("RUSTC_WRAPPER");
    command
}

/// The reference fixture is one mutable build tree: two tests driving it
/// concurrently would see each other's rebuild as a fresh compile and get no
/// documents, so they take turns.
static REFERENCE_FIXTURE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Remove every workspace package's build state, forcing the next check to
/// re-invoke rustowlc. Leaves `target_dir/cache` alone.
async fn clean_workspace_packages(cargo: &str, fixture: &Path, target_dir: &Path) {
    for package in workspace_packages(cargo, fixture).await {
        let mut clean = rustowl_cargo_command(env!("CARGO_BIN_EXE_rustowlc"), target_dir).await;
        clean
            .args(["clean", "--package", package.as_str()])
            .current_dir(fixture)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let output = clean.output().await.expect("cargo clean must run");
        assert!(
            output.status.success(),
            "cargo clean --package {package} failed ({})\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Analyse the perf fixture, returning every `Workspace` document rustowlc printed,
/// accumulated raw, plus the document count. Not via `Workspace::merge`:
/// merging dedups by `fn_id` before `normalize_items` can assert on a repeated delivery.
async fn analyze_perf_fixture(fixture: &Path) -> (RawWorkspace, usize) {
    let rustowlc = env!("CARGO_BIN_EXE_rustowlc");
    let cargo = toolchain::get_executable_path("cargo").await;
    let target_dir = fixture.join("target").join("owl-reference");

    tokio::fs::remove_dir_all(target_dir.join("cache"))
        .await
        .ok();

    clean_workspace_packages(&cargo, fixture, &target_dir).await;

    let mut command = rustowl_cargo_command(rustowlc, &target_dir).await;
    command
        .args([
            "check",
            "--workspace",
            "--keep-going",
            "--message-format=json",
        ])
        .current_dir(fixture)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = command.output().await.expect("cargo check must run");
    let stdout = String::from_utf8(output.stdout).expect("rustowlc must print UTF-8");
    assert!(
        output.status.success(),
        "cargo check over the perf fixture failed ({})\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    // Cargo's `--message-format=json` shares stdout; `File` requires a top-level
    // `items` field, which no cargo message carries.
    let mut documents = 0;
    let mut raw = RawWorkspace::new();
    for line in stdout.lines() {
        if let Ok(workspace) = serde_json::from_str::<Workspace>(line) {
            documents += 1;
            accumulate(&mut raw, workspace);
        }
    }
    assert!(
        documents > 0,
        "rustowlc printed no Workspace documents\nstdout:\n{stdout}"
    );
    (raw, documents)
}

/// One Workspace document per crate, not one per analysed function.
#[tokio::test]
async fn workspace_message_count_matches_packages() {
    require_toolchain_sysroot();
    let _reference_fixture = REFERENCE_FIXTURE.lock().await;

    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "perf-tests", "dummy-package"]
        .iter()
        .collect();
    let fixture = fixture.canonicalize().expect("the perf fixture must exist");

    let (raw, documents) = analyze_perf_fixture(&fixture).await;
    let analysed: usize = raw
        .values()
        .flat_map(|files| files.values())
        .map(Vec::len)
        .sum();

    assert_eq!(
        documents, REFERENCE_CRATES,
        "rustowlc must emit one Workspace line per crate ({REFERENCE_CRATES}), got {documents}"
    );
    assert_eq!(analysed, REFERENCE_FUNCTION_COUNT);
}

/// The reference output: everything `rustowl check` computes for the perf
/// fixture, in canonical form. Every later change is a no-behaviour-change
/// refactor verified by diffing against this.
///
/// Valid only for the pinned toolchain: `rust-toolchain.toml`'s nightly builds
/// `rustowlc`, while `scripts/toolchain`'s sysroot checks the fixture. Recorded names
/// are compiler-generated, so a rustc bump changes them — regenerate and review.
#[tokio::test]
async fn test_workspace_reference() {
    require_toolchain_sysroot();
    let _reference_fixture = REFERENCE_FIXTURE.lock().await;

    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "perf-tests", "dummy-package"]
        .iter()
        .collect();
    let fixture = fixture.canonicalize().expect("the perf fixture must exist");

    let reference = normalize_workspace(analyze_perf_fixture(&fixture).await.0, &fixture);

    let counted = reference
        .values()
        .flat_map(|files| files.iter())
        .map(|(file, items)| format!("{file}={}", items.len()))
        .collect::<Vec<_>>()
        .join(", ");
    let analysed: usize = reference
        .values()
        .flat_map(|files| files.values())
        .map(Vec::len)
        .sum();
    assert_eq!(
        analysed, REFERENCE_FUNCTION_COUNT,
        "expected {REFERENCE_FUNCTION_COUNT} analysed functions in the perf fixture, \
         got {analysed}: {counted}"
    );

    insta::assert_snapshot!(render_reference(&reference));
}

/// Render the reference output with one compact line per function.
///
/// Pretty-printed JSON put every range on its own line, which made the stored
/// snapshot ~103k lines for 74 functions and swamped any diff. The content is
/// identical -- only the framing changes -- so a behaviour change now shows up
/// as one changed line naming one function.
fn render_reference(reference: &ReferenceWorkspace) -> String {
    let mut out = String::new();
    for (crate_name, files) in reference {
        out.push_str(&format!("{crate_name}:\n"));
        for (file, functions) in files {
            out.push_str(&format!("  {file}:\n"));
            for function in functions {
                out.push_str(&format!(
                    "    {}\n",
                    serde_json::to_string(function).expect("a function must serialize")
                ));
            }
        }
    }
    out
}

/// Analyse the fixture with `RUST_LOG=debug`, returning the driver's stderr.
/// With `use_cache` false the cache is disabled entirely.
async fn check_fixture_for_cache(fixture: &Path, target_dir: &Path, use_cache: bool) -> String {
    let mut command = rustowl_cargo_command(env!("CARGO_BIN_EXE_rustowlc"), target_dir).await;
    if !use_cache {
        command.env_remove("RUSTOWL_CACHE_DIR");
    }
    command
        .env("RUST_LOG", "debug")
        .args([
            "check",
            "--workspace",
            "--keep-going",
            "--message-format=json",
        ])
        .current_dir(fixture)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = command.output().await.expect("cargo check must run");
    assert!(
        output.status.success(),
        "cargo check over the perf fixture failed ({})\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn count_cache_hits(stderr: &str) -> usize {
    stderr.matches("MIR cache hit").count()
}

/// `test_workspace_reference` wipes the cache on every run, so the warm path needs its own test.
#[tokio::test]
async fn warm_run_serves_bodies_from_cache() {
    require_toolchain_sysroot();

    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "perf-tests", "dummy-package"]
        .iter()
        .collect();
    let fixture = fixture.canonicalize().expect("the perf fixture must exist");
    let target_dir = fixture.join("target").join("owl-warm");

    tokio::fs::remove_dir_all(&target_dir).await.ok();

    let cargo = toolchain::get_executable_path("cargo").await;

    clean_workspace_packages(&cargo, &fixture, &target_dir).await;
    let cold = check_fixture_for_cache(&fixture, &target_dir, true).await;
    assert_eq!(
        count_cache_hits(&cold),
        0,
        "a cold cache must produce no hits"
    );

    let mut entries = tokio::fs::read_dir(target_dir.join("cache"))
        .await
        .expect("a cold run must write the cache directory");
    assert!(
        entries.next_entry().await.expect("cache dir").is_some(),
        "a cold run must leave at least one cache file behind"
    );

    clean_workspace_packages(&cargo, &fixture, &target_dir).await;
    let warm = check_fixture_for_cache(&fixture, &target_dir, true).await;
    assert!(
        count_cache_hits(&warm) > 0,
        "an unchanged warm run served nothing from the cache\n{}",
        warm.lines().rev().take(40).collect::<Vec<_>>().join("\n")
    );
}

/// The control for the test above: without it a cache that is never consulted
/// still passes, because "no error" and "hit every time" look the same.
#[tokio::test]
async fn no_cache_dir_means_no_hits() {
    require_toolchain_sysroot();

    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "perf-tests", "dummy-package"]
        .iter()
        .collect();
    let fixture = fixture.canonicalize().expect("the perf fixture must exist");
    let target_dir = fixture.join("target").join("owl-no-cache");

    tokio::fs::remove_dir_all(&target_dir).await.ok();

    let cargo = toolchain::get_executable_path("cargo").await;
    clean_workspace_packages(&cargo, &fixture, &target_dir).await;
    let first = check_fixture_for_cache(&fixture, &target_dir, false).await;
    clean_workspace_packages(&cargo, &fixture, &target_dir).await;
    let second = check_fixture_for_cache(&fixture, &target_dir, false).await;

    assert_eq!(
        count_cache_hits(&first) + count_cache_hits(&second),
        0,
        "a disabled cache must never report a hit"
    );
}
