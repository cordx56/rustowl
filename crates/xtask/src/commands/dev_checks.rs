use anyhow::{Context, Result, anyhow};
use clap::Parser;

use crate::util::{Cmd, read_to_string, repo_root};

#[derive(Parser, Debug)]
#[command(
    about = "Run developer checks (fmt, clippy, build, tests)",
    long_about = "Runs the project's standard developer quality checks:
- rustfmt (optionally fix)
- clippy (all targets, all features, workspace, -D warnings)
- stable rustc version check (>= `.rust-version-stable`)
- release build via `cargo xtask toolchain cargo build --release`
- a basic `cargo test --lib --bins`
- optional VSCode extension checks (if `vscode/` exists and `pnpm` is installed)"
)]
pub struct Args {
    /// Automatically fix issues where possible
    #[arg(short, long)]
    fix: bool,
}

pub async fn run(args: Args) -> Result<()> {
    let root = repo_root()?;

    if args.fix {
        Cmd::new("cargo").arg("fmt").cwd(&root).run().await?;
    } else {
        Cmd::new("cargo")
            .args(["fmt", "--check", "--all"])
            .cwd(&root)
            .run()
            .await?;
    }

    if args.fix {
        // Best-effort: clippy --fix can fail on some toolchains/configs.
        let _ = Cmd::new("cargo")
            .args(["clippy", "--fix", "--allow-dirty", "--allow-staged"])
            .cwd(&root)
            .run()
            .await;
    }

    Cmd::new("cargo")
        .args([
            "clippy",
            "--all-targets",
            "--all-features",
            "--workspace",
            "--",
            "-D",
            "warnings",
        ])
        .cwd(&root)
        .run()
        .await
        .context("clippy")?;

    check_stable_rust_min_version(&root).await?;

    // Build (release) using the custom toolchain wrapper.
    Cmd::new("cargo")
        .args(["xtask", "toolchain", "cargo", "build", "--release"])
        .cwd(&root)
        .run()
        .await
        .context("build")?;

    // Tests: keep parity with the previous script (run basic tests; project may have none).
    let output = Cmd::new("cargo")
        .args(["test", "--lib", "--bins"])
        .cwd(&root)
        .output()
        .await
        .context("cargo test")?;

    if !output.status.success() {
        return Err(anyhow!("cargo test failed"));
    }

    // VSCode checks, only if pnpm exists
    if root.join("vscode").is_dir() && crate::util::which("pnpm").is_some() {
        let vscode = root.join("vscode");
        if !vscode.join("node_modules").is_dir() {
            Cmd::new("pnpm")
                .args(["install", "--frozen-lockfile"])
                .cwd(&vscode)
                .run()
                .await
                .context("pnpm install")?;
        }
        if args.fix {
            let _ = Cmd::new("pnpm")
                .args(["prettier", "--write", "src"])
                .cwd(&vscode)
                .run()
                .await;
        } else {
            Cmd::new("pnpm")
                .args(["prettier", "--check", "src"])
                .cwd(&vscode)
                .run()
                .await
                .context("pnpm prettier")?;
        }
        Cmd::new("pnpm")
            .args(["lint"])
            .cwd(&vscode)
            .run()
            .await
            .context("pnpm lint")?;
        Cmd::new("pnpm")
            .args(["check-types"])
            .cwd(&vscode)
            .run()
            .await
            .context("pnpm check-types")?;
    }

    Ok(())
}

async fn check_stable_rust_min_version(root: &std::path::Path) -> Result<()> {
    // Parity with `scripts/dev-checks.sh`: require stable rustc >= `.rust-version-stable`.
    // This avoids surprising compiler errors when running release builds.
    let pinned =
        read_to_string(root.join(".rust-version-stable")).context("read .rust-version-stable")?;
    let pinned = pinned.trim();
    if pinned.is_empty() {
        return Ok(());
    }

    let output = Cmd::new("rustc")
        .args(["--version"])
        .cwd(root)
        .output()
        .await
        .context("rustc --version")?;

    let version_str = String::from_utf8_lossy(&output.stdout);
    let mut it = version_str.split_whitespace();
    let _rustc = it.next();
    let current = it.next().unwrap_or("").trim();

    if current.is_empty() {
        return Err(anyhow!("could not parse rustc version from: {version_str}"));
    }

    if compares_ge_semver(current, pinned) {
        Ok(())
    } else {
        Err(anyhow!(
            "rustc {current} is below required stable {pinned} (from .rust-version-stable)"
        ))
    }
}

fn compares_ge_semver(current: &str, required: &str) -> bool {
    // Minimal semver (x.y.z) comparison. Keep it local to xtask to avoid adding deps.
    // Accept inputs like `1.92.0` and `1.94.0-nightly`.
    fn parse(v: &str) -> Option<(u64, u64, u64)> {
        let v = v.split_once('-').map(|(a, _)| a).unwrap_or(v);
        let mut it = v.split('.');
        Some((
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
        ))
    }

    let Some(c) = parse(current) else {
        return false;
    };
    let Some(r) = parse(required) else {
        return false;
    };
    c >= r
}
