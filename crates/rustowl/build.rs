use clap::CommandFactory;
use clap_complete::generate_to;
use std::env;
use std::fs;
use std::io::Error;
use std::path::PathBuf;
use std::process::Command;

include!("src/cli.rs");
include!("src/shells.rs");

fn main() -> Result<(), Error> {
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
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    std::fs::write(man_out_dir.join("rustowl.1"), buffer)?;

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
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    Some(format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    ))
}

/// Converts a count of days since 1970-01-01 into a proleptic Gregorian date.
///
/// Uses Howard Hinnant's `civil_from_days`, which is exact across the whole
/// `i64` range and needs no lookup table.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Shift the epoch to 0000-03-01 so leap days land at the end of the cycle.
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // [0, 399]
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // [0, 365]
    let mp = (5 * day_of_year + 2) / 153; // [0, 11], March-based
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if month <= 2 { year + 1 } else { year }, month, day)
}
