use clap::CommandFactory;
use clap_complete::generate_to;
use jiff::Timestamp;
use std::env;
use std::fs;
use std::io::Error;
use std::path::PathBuf;
use std::process::Command;

include!("src/cli.rs");
include!("src/shells.rs");
include!("src/man.rs");

fn main() -> Result<(), Error> {
    // The script reads the CLI and the shell list, both under `src`. Saying so
    // keeps cargo from fingerprinting the whole package, which includes the
    // perf fixture's build tree that the tests rewrite while they run.
    println!("cargo::rerun-if-changed=src");

    // Declare custom cfg flags to avoid warnings
    println!("cargo::rustc-check-cfg=cfg(miri)");

    let toolchain = get_toolchain();
    println!("cargo::rustc-env=RUSTOWL_TOOLCHAIN={toolchain}");
    println!("cargo::rustc-env=TOOLCHAIN_CHANNEL={}", get_channel());
    if let Some(date) = get_toolchain_date() {
        println!("cargo::rustc-env=TOOLCHAIN_DATE={date}");
    }

    let host_tuple = get_host_tuple();
    println!("cargo::rustc-env=HOST_TUPLE={host_tuple}");

    emit_rerun_conditions();
    emit_build_provenance();

    #[cfg(target_os = "macos")]
    {
        println!("cargo::rustc-link-arg-bin=rustowlc=-Wl,-rpath,@executable_path/../lib");
    }
    #[cfg(target_os = "linux")]
    {
        println!("cargo::rustc-link-arg-bin=rustowlc=-Wl,-rpath,$ORIGIN/../lib");
    }
    #[cfg(target_os = "windows")]
    {
        println!("cargo::rustc-link-arg-bin=rustowlc=/LIBPATH:..\\bin");
    }

    let out_dir =
        std::path::Path::new(&env::var("OUT_DIR").expect("OUT_DIR unset. Expected path."))
            .join("rustowl-build-time-out");
    let mut cmd = Cli::command();
    let completion_out_dir = out_dir.join("completions");
    fs::create_dir_all(&completion_out_dir)?;

    for shell in Shell::value_variants() {
        generate_to(*shell, &mut cmd, "rustowl", &completion_out_dir)?;
    }
    let man_out_dir = out_dir.join("man");
    fs::create_dir_all(&man_out_dir)?;

    fs::write(man_out_dir.join("rustowl.1"), manpage().render())?;

    Ok(())
}

// get toolchain
fn get_toolchain() -> String {
    if let Ok(v) = env::var("RUSTUP_TOOLCHAIN") {
        v
    } else if let Ok(v) = env::var("TOOLCHAIN_CHANNEL") {
        format!("{v}-{}", get_host_tuple())
    } else {
        let rustc_v = rustc_release().expect("failed to get rustc version");
        format!("{rustc_v}-{}", get_host_tuple())
    }
}
fn get_channel() -> String {
    let toolchain = get_toolchain();

    if toolchain.contains("-nightly-") {
        "nightly".to_string()
    } else if toolchain.contains("-beta") {
        "beta".to_string()
    } else {
        toolchain
            .split('-')
            .next()
            .expect("failed to obtain channel from toolchain")
            .to_owned()
    }
}
fn get_toolchain_date() -> Option<String> {
    let r = regex::Regex::new(r#"\d\d\d\d-\d\d-\d\d"#).unwrap();
    r.find(&get_toolchain()).map(|v| v.as_str().to_owned())
}
fn get_host_tuple() -> String {
    rustc_output(&["--print", "host-tuple"]).expect("failed to obtain host-tuple")
}

/// Bakes build provenance into the binary for `--version` to report.
fn emit_build_provenance() {
    let provenance = [
        ("GIT_TAG", git_output(&["describe", "--tags", "--abbrev=0"])),
        (
            "GIT_COMMIT_HASH",
            git_output(&["rev-parse", "--short", "HEAD"]),
        ),
        ("BUILD_TIME", build_time()),
        ("RUSTC_VERSION", rustc_output(&["--version"])),
    ];
    for (key, value) in provenance {
        println!("cargo::rustc-env={key}={}", value.unwrap_or_default());
    }
}

/// Declares what makes the provenance above stale.
///
/// Git metadata is best-effort.
fn emit_rerun_conditions() {
    // Provenance inputs.
    println!("cargo::rerun-if-env-changed=SOURCE_DATE_EPOCH");
    // Inputs the implicit default used to cover.
    for path in ["src", "build.rs", "Cargo.toml"] {
        println!("cargo::rerun-if-changed={path}");
    }
    // Git inputs: HEAD moves on checkout/branch switch, the ref moves on commit.
    let Some(git_dir) = git_output(&["rev-parse", "--absolute-git-dir"]) else {
        return;
    };
    rerun_if_path_exists(std::path::Path::new(&git_dir).join("HEAD"));
    let common_dir = git_output(&["rev-parse", "--path-format=absolute", "--git-common-dir"]);
    let ref_name = git_output(&["symbolic-ref", "HEAD"]);
    if let (Some(common_dir), Some(ref_name)) = (common_dir, ref_name) {
        rerun_if_path_exists(std::path::Path::new(&common_dir).join(ref_name));
    }
}

/// Watches `path` only if it exists.
fn rerun_if_path_exists(path: PathBuf) {
    if path.exists() {
        println!("cargo::rerun-if-changed={}", path.display());
    }
}

/// Runs `git` and returns its trimmed stdout, or `None` if git is missing,
/// the command fails, or it produces no output.
fn git_output(args: &[&str]) -> Option<String> {
    command_output("git", args)
}

/// Runs the active compiler and returns its trimmed stdout.
fn rustc_output(args: &[&str]) -> Option<String> {
    command_output(&env::var("RUSTC").unwrap_or("rustc".to_string()), args)
}

/// The active compiler's version, as it reports itself.
fn rustc_release() -> Option<String> {
    rustc_output(&["-vV"])?
        .lines()
        .find_map(|line| line.strip_prefix("release: "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Runs `program` and returns its trimmed stdout, or `None` if it cannot be
/// spawned, exits non-zero, or prints nothing.
fn command_output(program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Formats the build time as UTC.
fn build_time() -> Option<String> {
    let secs = match env::var("SOURCE_DATE_EPOCH") {
        Ok(epoch) => epoch.trim().parse::<u64>().ok()?,
        Err(_) => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs(),
    };
    let ts = Timestamp::from_second(secs as i64).ok()?;
    Some(format!("{} UTC", ts.strftime("%F %T")))
}
