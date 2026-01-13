use clap::CommandFactory;
use clap_complete::generate_to;
use std::env;
use std::fs;
use std::io::Error;
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
        // Fallback: parse channel from rust-toolchain.toml.
        let v = std::fs::read_to_string("./rust-toolchain.toml")
            .expect("there are no toolchain specifier");
        let channel = v
            .lines()
            .find_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix("channel")?.trim_start();
                let rest = rest.strip_prefix('=')?.trim();
                rest.strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .map(|s| s.to_string())
            })
            .expect("failed to parse toolchain channel");
        format!("{}-{}", channel.trim(), get_host_tuple())
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
    use jiff::{Unit, Zoned};

    let now = Zoned::now().in_tz("UTC").ok()?.round(Unit::Second).ok()?;
    Some(now.strftime("%Y-%m-%d %H:%M:%S UTC").to_string())
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
