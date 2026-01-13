use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde_json::Value;
use std::path::PathBuf;

use crate::util::{Cmd, ensure_tool, read_to_string, repo_root, write_string};

#[derive(Parser, Debug)]
#[command(
    about = "Bump versions and create a git tag",
    long_about = "Updates version fields for a release and creates an annotated git tag.

What gets updated:
- `crates/rustowl/Cargo.toml` version
- `vscode/package.json` version (if present)
- AUR PKGBUILD files (if present and not a prerelease)

Then runs: `git tag <version>`.

Example:
  cargo xtask bump v1.0.0"
)]
pub struct Args {
    /// Version tag like `v1.2.3` (must start with 'v')
    #[arg(value_name = "VERSION")]
    version: String,
}

pub async fn run(args: Args) -> Result<()> {
    let root = repo_root()?;
    ensure_tool("git")?;

    let (version_tag, version) = parse_version(&args.version)?;
    let is_prerelease = is_prerelease(&version);

    update_rustowl_cargo_toml(&root.join("crates/rustowl/Cargo.toml"), &version)?;

    let vscode_pkg = root.join("vscode/package.json");
    if vscode_pkg.is_file() {
        update_vscode_package_json(&vscode_pkg, &version)?;
    }

    if !is_prerelease {
        let aur_pkgbuild = root.join("aur/PKGBUILD");
        if aur_pkgbuild.is_file() {
            update_pkgbuild(&aur_pkgbuild, &version)?;
        }
        let aur_pkgbuild_bin = root.join("aur/PKGBUILD-BIN");
        if aur_pkgbuild_bin.is_file() {
            update_pkgbuild(&aur_pkgbuild_bin, &version)?;
        }
    }

    Cmd::new("git")
        .args(["tag", &version_tag])
        .cwd(&root)
        .run()
        .await
        .context("git tag")?;

    Ok(())
}

fn parse_version(input: &str) -> Result<(String, String)> {
    if !input.starts_with('v') {
        return Err(anyhow!("version must start with 'v' (e.g. v0.3.1)"));
    }
    let ver = input.trim_start_matches('v').to_string();
    if ver.is_empty() {
        return Err(anyhow!("invalid version"));
    }
    Ok((input.to_string(), ver))
}

fn is_prerelease(version: &str) -> bool {
    let lower = version.to_ascii_lowercase();
    ["alpha", "beta", "rc", "dev", "pre", "snapshot"]
        .iter()
        .any(|p| lower.contains(p))
}

fn update_rustowl_cargo_toml(path: &PathBuf, version: &str) -> Result<()> {
    let original = read_to_string(path)?;
    let mut out = String::new();
    let mut replaced = false;

    for line in original.lines() {
        if !replaced && line.trim_start().starts_with("version =") {
            out.push_str(&format!("version = \"{}\"\n", version));
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    if !replaced {
        return Err(anyhow!("did not find version field in {}", path.display()));
    }

    write_string(path, &out)?;
    Ok(())
}

fn update_vscode_package_json(path: &PathBuf, version: &str) -> Result<()> {
    let content = read_to_string(path)?;
    let mut json: Value = serde_json::from_str(&content).context("parse vscode/package.json")?;
    json["version"] = Value::String(version.to_string());
    let formatted = serde_json::to_string_pretty(&json).context("serialize vscode/package.json")?;
    write_string(path, &(formatted + "\n"))?;
    Ok(())
}

fn update_pkgbuild(path: &PathBuf, version: &str) -> Result<()> {
    let original = read_to_string(path)?;
    let mut out = String::new();
    let mut replaced = false;
    for line in original.lines() {
        if line.starts_with("pkgver=") {
            out.push_str(&format!("pkgver={}\n", version));
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        return Err(anyhow!("did not find pkgver= in {}", path.display()));
    }
    write_string(path, &out)?;
    Ok(())
}
