use clap::CommandFactory;
use clap_complete::generate_to;
use std::env;
use std::fs;
use std::io::Error;
use std::process::Command;
use std::time::SystemTime;

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

    // Git information for detailed version output
    // Always set these env vars (empty string if not found, handled at runtime)
    println!(
        "cargo::rustc-env=GIT_TAG={}",
        get_git_tag().unwrap_or_default()
    );
    println!(
        "cargo::rustc-env=GIT_COMMIT_HASH={}",
        get_git_commit_hash().unwrap_or_default()
    );
    println!(
        "cargo::rustc-env=BUILD_TIME={}",
        get_build_time().unwrap_or_default()
    );
    println!(
        "cargo::rustc-env=RUSTC_VERSION={}",
        get_rustc_version().unwrap_or_default()
    );

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
        let v = std::fs::read_to_string("./scripts/build/channel")
            .expect("there are no toolchain specifier");
        format!("{}-{}", v.trim(), get_host_tuple())
    }
}
fn get_channel() -> String {
    get_toolchain()
        .split("-")
        .next()
        .expect("failed to obtain channel from toolchain")
        .to_owned()
}
fn get_toolchain_date() -> Option<String> {
    let r = regex::Regex::new(r#"\d\d\d\d-\d\d-\d\d"#).unwrap();
    r.find(&get_toolchain()).map(|v| v.as_str().to_owned())
}
fn get_host_tuple() -> String {
    Command::new(env::var("RUSTC").unwrap_or("rustc".to_string()))
        .arg("--print")
        .arg("host-tuple")
        .output()
        .map(|v| String::from_utf8(v.stdout).unwrap().trim().to_string())
        .expect("failed to obtain host-tuple")
}

fn get_git_tag() -> Option<String> {
    Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
}

fn get_git_commit_hash() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
}

fn get_build_time() -> Option<String> {
    // Cross-platform build time using SystemTime
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| {
            // Convert to a simple timestamp format
            let secs = d.as_secs();
            // Calculate date components (simplified UTC)
            let days = secs / 86400;
            let time_secs = secs % 86400;
            let hours = time_secs / 3600;
            let mins = (time_secs % 3600) / 60;
            let secs = time_secs % 60;

            // Days since 1970-01-01
            let mut y = 1970;
            let mut remaining_days = days;

            loop {
                let days_in_year = if is_leap_year(y) { 366 } else { 365 };
                if remaining_days < days_in_year {
                    break;
                }
                remaining_days -= days_in_year;
                y += 1;
            }

            let month_days: [u64; 12] = if is_leap_year(y) {
                [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
            } else {
                [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
            };

            let mut m = 1;
            for days_in_month in month_days {
                if remaining_days < days_in_month {
                    break;
                }
                remaining_days -= days_in_month;
                m += 1;
            }

            let d = remaining_days + 1;

            format!("{y:04}-{m:02}-{d:02} {hours:02}:{mins:02}:{secs:02} UTC")
        })
}

fn is_leap_year(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn get_rustc_version() -> Option<String> {
    Command::new(env::var("RUSTC").unwrap_or("rustc".to_string()))
        .args(["--version"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
}
