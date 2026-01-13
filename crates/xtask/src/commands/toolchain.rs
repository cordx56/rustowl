use anyhow::{Context, Result, anyhow};
use clap::Parser;
use flate2::read::GzDecoder;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use tar::Archive;
use tempfile::TempDir;

use crate::util::{Cmd, read_to_string, repo_root, which};

#[derive(Parser, Debug)]
#[command(
    about = "Run a command using RustOwl's pinned toolchain/sysroot",
    long_about = "Runs any command with RustOwl's pinned Rust toolchain available on PATH.

This command downloads a minimal sysroot (rustc, rust-std, cargo, rustc-dev, llvm-tools)
into `~/.rustowl/sysroot/<channel>-<host>/` (or `$SYSROOT` if set) and then executes
the requested command with that sysroot's `bin/` prepended to PATH.

Common usage is wrapping `cargo` so CI and local tooling use the same compiler bits.

Examples:
  cargo xtask toolchain cargo build --release
  cargo xtask toolchain cargo test -p rustowl
  cargo xtask toolchain cargo +nightly miri test -p rustowl"
)]
pub struct Args {
    /// Command (and args) to execute under the RustOwl sysroot
    #[arg(trailing_var_arg = true, required = true, value_name = "CMD")]
    cmd: Vec<OsString>,
}

pub async fn run(args: Args) -> Result<()> {
    let root = repo_root()?;

    let channel = read_toolchain_channel(&root)?;
    let host = host_tuple()?;
    let toolchain = format!("{}-{}", channel, host);

    let sysroot = match std::env::var_os("SYSROOT") {
        Some(s) => PathBuf::from(s),
        None => {
            let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME not set"))?;
            PathBuf::from(home)
                .join(".rustowl/sysroot")
                .join(&toolchain)
        }
    };

    ensure_sysroot(&sysroot, &toolchain).await?;

    let mut iter = args.cmd.into_iter();
    let program = iter
        .next()
        .ok_or_else(|| anyhow!("missing command"))?
        .to_string_lossy()
        .to_string();
    let cmd_args: Vec<String> = iter.map(|s| s.to_string_lossy().to_string()).collect();

    let path = sysroot.join("bin");
    let existing = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", path.display(), existing);

    Cmd::new(program)
        .args(cmd_args)
        .cwd(&root)
        .env("PATH", new_path)
        .env("RUSTC_BOOTSTRAP", "rustowlc")
        .run()
        .await
}

fn read_toolchain_channel(root: &Path) -> Result<String> {
    let pinned_stable = root.join(".rust-version-stable");
    if pinned_stable.is_file() {
        return Ok(read_to_string(&pinned_stable)?
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string());
    }

    Err(anyhow!(
        "could not locate pinned stable toolchain version (expected .rust-version-stable)"
    ))
}

fn host_tuple() -> Result<String> {
    let os = if cfg!(target_os = "linux") {
        "unknown-linux-gnu"
    } else if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "windows") {
        "pc-windows-msvc"
    } else {
        return Err(anyhow!("unsupported OS"));
    };

    let hint = std::env::var("RUNNER_ARCH")
        .ok()
        .or_else(|| std::env::var("PROCESSOR_ARCHITEW6432").ok())
        .or_else(|| std::env::var("PROCESSOR_ARCHITECTURE").ok())
        .or_else(|| std::env::var("MSYSTEM_CARCH").ok());

    let arch = match hint.as_deref() {
        Some("ARM64") | Some("arm64") | Some("aarch64") => "aarch64",
        Some("AMD64") | Some("X64") | Some("amd64") | Some("x86_64") => "x86_64",
        _ => {
            let arch = std::env::consts::ARCH;
            match arch {
                "aarch64" => "aarch64",
                "x86_64" => "x86_64",
                other => return Err(anyhow!("unsupported architecture: {other}")),
            }
        }
    };

    Ok(format!("{arch}-{os}"))
}

async fn ensure_sysroot(sysroot: &Path, toolchain: &str) -> Result<()> {
    if sysroot.is_dir() {
        return Ok(());
    }

    std::fs::create_dir_all(sysroot)
        .with_context(|| format!("create sysroot {}", sysroot.display()))?;

    let components = ["rustc", "rust-std", "cargo", "rustc-dev", "llvm-tools"];

    // Download/install in parallel (matches legacy shell script behavior).
    let mut tasks = Vec::new();
    for component in components {
        tasks.push(tokio::spawn(install_component(
            component,
            sysroot.to_path_buf(),
            toolchain.to_string(),
        )));
    }

    for t in tasks {
        t.await.context("join toolchain installer")??;
    }

    Ok(())
}

async fn install_component(component: &str, sysroot: PathBuf, toolchain: String) -> Result<()> {
    let dist_base = "https://static.rust-lang.org/dist";
    let url = format!("{dist_base}/{component}-{toolchain}.tar.gz");
    eprintln!("Downloading {url}");

    let resp = reqwest::get(&url)
        .await
        .with_context(|| format!("GET {url}"))?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(anyhow!(
            "toolchain artifact not found (404): {url}\n\
This usually means the pinned nightly ({toolchain}) is no longer available on static.rust-lang.org (cleanup/retention).\n\
Fix by updating `rust-toolchain.toml` to an existing nightly date or set `$SYSROOT` to a pre-downloaded sysroot."
        ));
    }

    let bytes = resp
        .error_for_status()
        .with_context(|| format!("HTTP {url}"))?
        .bytes()
        .await
        .with_context(|| format!("read body {url}"))?;

    let temp = TempDir::new().context("tempdir")?;
    let tar = GzDecoder::new(bytes.as_ref());
    let mut archive = Archive::new(tar);
    archive.unpack(temp.path()).context("unpack")?;

    let component_dir = format!("{component}-{toolchain}");
    let base = temp.path().join(&component_dir);
    let components_file = base.join("components");
    let comps = std::fs::read_to_string(&components_file)
        .with_context(|| format!("read {}", components_file.display()))?;

    for entry in comps.lines().filter(|l| !l.trim().is_empty()) {
        let com_base = base.join(entry.trim());
        let files_dir = com_base;
        if !files_dir.is_dir() {
            continue;
        }
        // Mirror the old script: move all files into sysroot.
        for path in walk_files(&files_dir)? {
            let rel = path.strip_prefix(&files_dir).unwrap();
            let dest = sysroot.join(rel);
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p).with_context(|| format!("mkdir {}", p.display()))?;
            }
            std::fs::rename(&path, &dest).or_else(|_| {
                std::fs::copy(&path, &dest)
                    .map(|_| ())
                    .with_context(|| format!("copy {}", path.display()))
            })?;
        }
    }

    Ok(())
}

fn walk_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_files_inner(dir, &mut out)?;
    Ok(out)
}

fn walk_files_inner(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry.context("read_dir entry")?;
        let path = entry.path();
        let ty = entry.file_type().context("file_type")?;
        if ty.is_dir() {
            walk_files_inner(&path, out)?;
        } else if ty.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn ensure_git() -> Result<()> {
    if which("git").is_none() {
        return Err(anyhow!("git not found"));
    }
    Ok(())
}
