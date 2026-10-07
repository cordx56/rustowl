use rustowl::models::{Crate, Function, MirDecl, Workspace};
use rustowl::toolchain;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Once;

static BUILD_ONCE: Once = Once::new();

fn ensure_rustowl_built() {
    BUILD_ONCE.call_once(|| {
        let mut cmd = Command::new("cargo");
        if cfg!(windows) {
            cmd.args(["build", "--profile", "windows-release"]);
        } else {
            cmd.args(["build", "--release"]);
        }
        let output = cmd
            .output()
            .unwrap_or_else(|e| panic!("Failed to execute cargo build: {e}"));
        assert!(
            output.status.success(),
            "Failed to build rustowl.\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    });
}

fn get_rustowl_output(function_path: &str, variable: &str) -> String {
    ensure_rustowl_built();

    let module = function_path
        .split("::")
        .next()
        .expect("function path must start with a module name");

    // Absolute path to the binary of this package (CWD-independent: test
    // runners execute with varying working directories in a workspace).
    let rustowl_path = env!("CARGO_BIN_EXE_rustowl");

    let fixture_path: std::path::PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "algo-tests",
        "src",
        &format!("{module}.rs"),
    ]
    .iter()
    .collect();

    let output = Command::new(rustowl_path)
        .args([
            "show",
            "--path",
            fixture_path.to_str().expect("fixture path must be UTF-8"),
            function_path,
            variable,
        ])
        .output()
        .unwrap_or_else(|e| panic!("Failed to execute {rustowl_path}: {e}"));

    assert!(
        output.status.success(),
        "{rustowl_path} command failed.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("Invalid UTF-8")
}

#[test]
fn test_f1_v1() {
    let output = get_rustowl_output("vec::f1", "v1");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f1_v2() {
    let output = get_rustowl_output("vec::f1", "v2");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f2_v1() {
    let output = get_rustowl_output("vec::f2", "v1");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f2_v2() {
    let output = get_rustowl_output("vec::f2", "v2");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f3_v1() {
    let output = get_rustowl_output("vec::f3", "v1");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f3_v2() {
    let output = get_rustowl_output("vec::f3", "v2");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f4_v1() {
    let output = get_rustowl_output("vec::f4", "v1");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f5_r() {
    let output = get_rustowl_output("vec::f5", "r");
    insta::assert_snapshot!(output);
}

// must_live soundness check
#[test]
fn test_must_live_reassign_block_a() {
    let output = get_rustowl_output("must_live::reassign_block", "a");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_reassign_move_a() {
    let output = get_rustowl_output("must_live::reassign_move", "a");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_loop_local_s() {
    let output = get_rustowl_output("must_live::loop_local", "s");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_macro_use_m() {
    let output = get_rustowl_output("must_live::macro_use", "m");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_deref_use_m() {
    let output = get_rustowl_output("must_live::deref_use", "m");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_call_use_m() {
    let output = get_rustowl_output("must_live::call_use", "m");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_copy_use_m() {
    let output = get_rustowl_output("must_live::copy_use", "m");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_move_while_borrowed_a() {
    let output = get_rustowl_output("must_live::move_while_borrowed", "a");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_drop_use_s() {
    let output = get_rustowl_output("must_live::drop_use", "s");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_nll_v() {
    let output = get_rustowl_output("must_live::nll", "v");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_struct_borrow_s() {
    let output = get_rustowl_output("must_live::struct_borrow", "s");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_guard_m() {
    let output = get_rustowl_output("must_live::guard", "m");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_in_loop_s() {
    let output = get_rustowl_output("must_live::in_loop", "s");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_cond_init_v() {
    let output = get_rustowl_output("must_live::cond_init", "v");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_cond_move_a() {
    let output = get_rustowl_output("must_live::cond_move", "a");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_reinit_a() {
    let output = get_rustowl_output("must_live::reinit", "a");
    insta::assert_snapshot!(output);
}

#[test]
fn test_fixpoint_many_arms_s11() {
    let output = get_rustowl_output("fixpoint::many_arms", "s11");
    insta::assert_snapshot!(output);
}

#[test]
fn test_places_cond_partial_move_p() {
    let output = get_rustowl_output("places::cond_partial_move", "p");
    insta::assert_snapshot!(output);
}

#[test]
fn test_places_reinit_field_p() {
    let output = get_rustowl_output("places::reinit_field", "p");
    insta::assert_snapshot!(output);
}

#[test]
fn test_places_assign_field_after_move_p() {
    let output = get_rustowl_output("places::assign_field_after_move", "p");
    insta::assert_snapshot!(output);
}

#[test]
fn test_places_move_out_of_box_b() {
    let output = get_rustowl_output("places::move_out_of_box", "b");
    insta::assert_snapshot!(output);
}

/// Functions `cargo check --workspace` analyses in the perf fixture with default
/// features: the `rustowl_perf_test_dummy` lib plus the `dummy-app` bin.
const GOLDEN_FUNCTION_COUNT: usize = 74;

/// Canonical form of a merged [`Workspace`]: crate -> file -> functions.
type GoldenWorkspace = BTreeMap<String, BTreeMap<String, Vec<Function>>>;

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

    for ranges in [
        lives,
        shared_borrow,
        mutable_borrow,
        drop_range,
        definitely_live_at,
        maybe_init_at,
        deficit_at,
        storage_range,
    ] {
        ranges.sort_by_key(|range| (range.from().0, range.until().0));
    }
}

fn normalize_items(mut items: Vec<Function>) -> Vec<Function> {
    items.sort_by(|a, b| a.name.cmp(&b.name).then(a.fn_id.cmp(&b.fn_id)));

    let mut deduped: Vec<Function> = Vec::with_capacity(items.len());
    for item in items {
        if let Some(previous) = deduped.last()
            && previous.fn_id == item.fn_id
        {
            assert!(
                serde_json::to_string(previous).ok() == serde_json::to_string(&item).ok(),
                "fn_id {} ({}) was reported twice with different contents; collapsing \
                 them would hide the difference",
                item.fn_id,
                item.name
            );
        } else {
            deduped.push(item);
        }
    }

    for function in &mut deduped {
        function.decls.sort_by_key(decl_local_id);
        for decl in &mut function.decls {
            normalize_decl(decl);
        }
    }
    deduped
}

fn normalize_workspace(merged: Workspace, fixture_root: &Path) -> GoldenWorkspace {
    let Workspace(crates) = merged;
    let mut workspace = GoldenWorkspace::new();
    for (crate_name, Crate(files)) in crates {
        let mut per_file = BTreeMap::new();
        for (file, payload) in files {
            per_file.insert(
                relative_file_name(&file, fixture_root),
                normalize_items(payload.items),
            );
        }
        workspace.insert(crate_name, per_file);
    }
    workspace
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

/// A cargo command wired the way `rustowl check` wires it: `rustowlc` as the
/// compiler, and the wrapper's sysroot folded into the rustflags.
async fn rustowl_cargo_command(rustowlc: &str, target_dir: &Path) -> tokio::process::Command {
    let mut command = toolchain::setup_cargo_command().await;
    command
        .env("RUSTC", rustowlc)
        .env("RUSTC_WORKSPACE_WRAPPER", rustowlc)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTOWL_CACHE_DIR", target_dir.join("cache"))
        .env_remove("RUSTOWL_OPEN_FILES")
        .env_remove("RUSTC_WRAPPER");
    command
}

/// Analyse the perf fixture and return every `Workspace` document merged into one.
async fn analyze_perf_fixture(fixture: &Path) -> Workspace {
    let rustowlc = env!("CARGO_BIN_EXE_rustowlc");
    let cargo = toolchain::get_executable_path("cargo").await;
    let target_dir = fixture.join("target").join("owl-golden");

    tokio::fs::remove_dir_all(target_dir.join("cache"))
        .await
        .ok();

    for package in workspace_packages(&cargo, fixture).await {
        let mut clean = rustowl_cargo_command(rustowlc, &target_dir).await;
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

    let mut documents = 0;
    let mut merged = Workspace(HashMap::new());
    for line in stdout.lines() {
        if let Ok(workspace) = serde_json::from_str::<Workspace>(line) {
            documents += 1;
            merged.merge(workspace);
        }
    }
    assert!(
        documents > 0,
        "rustowlc printed no Workspace documents\nstdout:\n{stdout}"
    );
    merged
}

#[tokio::test]
async fn test_golden_workspace() {
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "perf-tests", "dummy-package"]
        .iter()
        .collect();
    let fixture = fixture.canonicalize().expect("the perf fixture must exist");

    let golden = normalize_workspace(analyze_perf_fixture(&fixture).await, &fixture);

    let counted = golden
        .values()
        .flat_map(|files| files.iter())
        .map(|(file, items)| format!("{file}={}", items.len()))
        .collect::<Vec<_>>()
        .join(", ");
    let analysed: usize = golden
        .values()
        .flat_map(|files| files.values())
        .map(Vec::len)
        .sum();
    assert_eq!(
        analysed, GOLDEN_FUNCTION_COUNT,
        "expected {GOLDEN_FUNCTION_COUNT} analysed functions in the perf fixture, \
         got {analysed}: {counted}"
    );

    insta::assert_json_snapshot!(golden);
}
