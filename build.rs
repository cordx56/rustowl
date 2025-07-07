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

    println!("cargo::rustc-env=RUSTOWL_TOOLCHAIN={}", get_toolchain());
    println!("cargo::rustc-env=TOOLCHAIN_CHANNEL={}", get_channel());
    if let Some(date) = get_toolchain_date() {
        println!("cargo::rustc-env=TOOLCHAIN_DATE={date}");
    }

    let host_tuple = get_host_tuple().expect("unable to obtain host-tuple");
    println!("cargo::rustc-env=HOST_TUPLE={host_tuple}");

    let tarball_name = if cfg!(windows) {
        format!("rustowl-{}.zip", get_host_tuple().unwrap())
    } else {
        format!("rustowl-{}.tar.gz", get_host_tuple().unwrap())
    };

    println!("cargo::rustc-env=RUSTOWL_ARCHIVE_NAME={tarball_name}");

    let sysroot = get_sysroot().unwrap();
    set_rustc_driver_path(&sysroot);

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
    env::var("RUSTUP_TOOLCHAIN").expect("RUSTUP_TOOLCHAIN unset. Expected version.")
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
fn get_host_tuple() -> Option<String> {
    match Command::new(env::var("RUSTC").unwrap_or("rustc".to_string()))
        .arg("--print")
        .arg("host-tuple")
        .output()
    {
        Ok(v) => Some(String::from_utf8(v.stdout).unwrap().trim().to_string()),
        Err(_) => None,
    }
}
// output rustc_driver path
fn get_sysroot() -> Option<String> {
    match Command::new(env::var("RUSTC").expect("RUSTC unset. Expected rustc path."))
        .arg("--print=sysroot")
        .output()
    {
        Ok(v) => Some(String::from_utf8(v.stdout).unwrap().trim().to_string()),
        Err(_) => None,
    }
}
use std::fs::read_dir;
use std::path::PathBuf;
fn recursive_read_dir(path: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(entries) = read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                paths.extend_from_slice(&recursive_read_dir(entry.path()));
            } else {
                paths.push(entry.path());
            }
        }
    }
    paths
}
fn set_rustc_driver_path(sysroot: &str) {
    for file in recursive_read_dir(sysroot) {
        if let Some(ext) = file.extension().and_then(|e| e.to_str())
            && matches!(ext, "rlib" | "so" | "dylib" | "dll")
            && let Ok(rel_path) = file.strip_prefix(sysroot)
            && let Some(file_name) = rel_path.file_name()
        {
            let file_name = file_name.to_string_lossy();
            if file_name.contains("rustc_driver-") {
                println!("cargo::rustc-env=RUSTC_DRIVER_NAME={file_name}");
            }
        }
    }
}
