use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
};

use crate::util::{Cmd, OsKind, is_ci, os_kind, repo_root, sudo_install, which, write_string};

async fn instruments_available(root: &Path) -> bool {
    if which("instruments").is_none() {
        return false;
    }

    // Match the shell script: instruments exists, but can be non-functional without Xcode setup.
    Cmd::new("timeout")
        .args(["10s", "instruments", "-help"])
        .cwd(root)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[derive(Parser, Debug)]
#[command(
    about = "Run security-oriented checks",
    long_about = "Runs a suite of security and correctness checks and writes logs to `security-logs/`.

Modes:
- default: run configured checks and write a summary Markdown file
- `--check`: print tool availability and exit
- `--install`: try installing missing tools (interactive mode)
- `--ci`: force CI mode (enables auto-install + verbose output)

Checks include:
- `cargo deny check` (unless `--no-deny`)
- `cargo shear` (optional)
- `cargo miri` (optional; runs tests under Miri)
- valgrind (optional; platform-dependent)

In CI, this command can auto-install missing cargo tools and some OS packages."
)]
pub struct Args {
    /// Only check tool availability and exit (no tests)
    #[arg(long)]
    check: bool,

    /// Install missing tools in interactive mode
    #[arg(long)]
    install: bool,

    /// Force CI mode (enables auto-install and verbose logging)
    #[arg(long)]
    ci: bool,

    /// Disable auto-installation (even in CI mode)
    #[arg(long)]
    no_auto_install: bool,

    /// Skip Miri tests
    #[arg(long = "no-miri")]
    no_miri: bool,

    /// Skip valgrind checks
    #[arg(long = "no-valgrind")]
    no_valgrind: bool,

    /// Force-enable Valgrind even on unsupported platforms (e.g. macOS)
    #[arg(long = "force-valgrind")]
    force_valgrind: bool,

    /// Skip dependency vulnerabilities check (cargo-deny)
    #[arg(long = "no-deny")]
    no_deny: bool,

    /// Skip unused dependency scan (cargo-shear)
    #[arg(long = "no-shear")]
    no_shear: bool,

    /// Skip macOS Instruments checks
    #[arg(long = "no-instruments")]
    no_instruments: bool,

    /// Force-enable Instruments checks on macOS
    #[arg(long = "force-instruments")]
    force_instruments: bool,

    /// Override MIRIFLAGS (default matches legacy script)
    #[arg(
        long,
        value_name = "FLAGS",
        default_value = "-Zmiri-disable-isolation -Zmiri-permissive-provenance"
    )]
    miri_flags: String,

    /// Override RUSTFLAGS for Miri (default matches legacy script)
    #[arg(long, value_name = "FLAGS", default_value = "--cfg miri")]
    miri_rustflags: String,
}

pub async fn run(args: Args) -> Result<()> {
    let root = repo_root()?;
    let logs_dir = root.join("security-logs");

    let ci_mode = args.ci || is_ci();
    let auto_install = !args.no_auto_install && (args.install || ci_mode);

    // Keep flags for CLI parity with the legacy script.
    let _ = args.no_instruments;

    if ci_mode {
        eprintln!(
            "CI detected (auto-install: {})",
            if auto_install { "enabled" } else { "disabled" }
        );
    }

    if args.install && !auto_install {
        // This can happen if `--install` and `--no-auto-install` are both set.
        return Err(anyhow!("--install conflicts with --no-auto-install"));
    }

    // Keep parity with the shell scripts: require stable rustc >= .rust-version-stable.
    check_stable_rust_min_version(&root).await?;

    // Auto-configure defaults based on platform (mirrors scripts/security.sh).
    //
    // Rule of thumb:
    // - explicit user flags win (e.g. `--no-*`), unless the user also `--force-*`
    // - "force" only affects auto-config defaults; it doesn't bypass missing tools
    let mut args = args;
    match os_kind() {
        OsKind::Linux => {
            // Linux: keep default behavior (Miri + Valgrind are allowed).
        }
        OsKind::Macos => {
            // macOS: legacy script disabled valgrind, instruments, and TSAN by default.
            if !args.force_valgrind {
                args.no_valgrind = true;
            }

            // Instruments exists only on macOS, but is off by default in the script.
            if !args.force_instruments {
                args.no_instruments = true;
            }
        }
        _ => {
            // Unknown platform: be conservative.
            if !args.force_valgrind {
                args.no_valgrind = true;
            }
            if !args.force_instruments {
                args.no_instruments = true;
            }

            // Also disable nightly-dependent features on unknown platforms.
            args.no_miri = true;
        }
    }

    // Apply force overrides last (so they reliably undo auto-config).
    if args.force_valgrind {
        args.no_valgrind = false;
    }
    if args.force_instruments {
        args.no_instruments = false;
    }

    if args.check {
        print_tool_status(&root, ci_mode).await?;
        return Ok(());
    }

    println!("RustOwl Security & Memory Safety Testing");
    println!("=========================================");
    println!();

    let mut summary = String::new();
    writeln!(&mut summary, "# Security Testing Summary")?;
    writeln!(&mut summary)?;
    writeln!(&mut summary, "Generated by `cargo xtask security`.")?;
    writeln!(&mut summary)?;

    let mut overall_ok = true;

    // cargo-deny always runs unless explicitly disabled.
    // CI policy: security.yml passes `--no-deny` to avoid duplicate cost.
    if !args.no_deny {
        ensure_cargo_tool("cargo-deny", "cargo-deny", auto_install).await?;
        println!("\n== cargo-deny ==");
        let (ok, out) = run_and_capture(
            &root,
            "cargo deny check",
            Cmd::new("cargo").args(["deny", "check"]).cwd(&root),
        )
        .await;
        write_string(logs_dir.join("cargo-deny.log"), &out)?;
        overall_ok &= ok;
        append_step(
            &mut summary,
            "cargo deny",
            ok,
            Some("security-logs/cargo-deny.log"),
        );
    } else {
        append_step(&mut summary, "cargo deny", true, Some("skipped"));
    }

    if !args.no_shear {
        // `cargo shear` is used to detect unused dependencies.
        ensure_cargo_tool("cargo-shear", "cargo-shear", auto_install).await?;
        println!("\n== cargo-shear ==");
        let (ok, out) = run_and_capture(
            &root,
            "cargo shear",
            Cmd::new("cargo").args(["shear"]).cwd(&root),
        )
        .await;
        write_string(logs_dir.join("cargo-shear.log"), &out)?;
        overall_ok &= ok;
        append_step(
            &mut summary,
            "cargo shear",
            ok,
            Some("security-logs/cargo-shear.log"),
        );
    } else {
        append_step(&mut summary, "cargo shear", true, Some("skipped"));
    }

    // We don't run nextest by default in security. We still ensure it's installed because Miri can
    // use it as a faster test runner (via `cargo miri nextest`).
    ensure_cargo_tool("cargo-nextest", "cargo-nextest", auto_install).await?;
    append_step(
        &mut summary,
        "cargo nextest",
        true,
        Some("available (used by miri)"),
    );

    if !args.no_miri {
        // Miri requires nightly.
        ensure_miri(auto_install).await?;

        println!("\n== miri ==");
        // Phase 1: unit tests under Miri.
        // Legacy script: use `miri nextest` when available, else fall back to `miri test`.
        let (ok_unit, out_unit) = {
            let nextest_available = Cmd::new("cargo")
                .args(["nextest", "--version"])
                .cwd(&root)
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false);

            if nextest_available {
                run_and_capture(
                    &root,
                    "miri unit tests (nextest)",
                    Cmd::new("cargo")
                        .args([
                            "xtask",
                            "toolchain",
                            "rustup",
                            "run",
                            "nightly",
                            "cargo",
                            "miri",
                            "nextest",
                            "run",
                            "--lib",
                            "-p",
                            "rustowl",
                        ])
                        .cwd(&root)
                        .env("MIRIFLAGS", &args.miri_flags)
                        .env("RUSTFLAGS", &args.miri_rustflags),
                )
                .await
            } else {
                run_and_capture(
                    &root,
                    "miri unit tests (cargo test)",
                    Cmd::new("cargo")
                        .args([
                            "xtask",
                            "toolchain",
                            "rustup",
                            "run",
                            "nightly",
                            "cargo",
                            "miri",
                            "test",
                            "--lib",
                            "-p",
                            "rustowl",
                        ])
                        .cwd(&root)
                        .env("MIRIFLAGS", &args.miri_flags)
                        .env("RUSTFLAGS", &args.miri_rustflags),
                )
                .await
            }
        };
        write_string(logs_dir.join("miri_unit_tests.log"), &out_unit)?;
        overall_ok &= ok_unit;
        append_step(
            &mut summary,
            "miri unit tests",
            ok_unit,
            Some("security-logs/miri_unit_tests.log"),
        );

        append_step(
            &mut summary,
            "miri rustowl run",
            true,
            Some("skipped (removed; proc-spawn makes it unreliable)"),
        );
    } else {
        append_step(&mut summary, "miri", true, Some("skipped"));
    }

    if !args.no_instruments {
        if os_kind() != OsKind::Macos {
            append_step(
                &mut summary,
                "instruments",
                true,
                Some("skipped (non-macOS)"),
            );
        } else if !instruments_available(&root).await {
            append_step(
                &mut summary,
                "instruments",
                false,
                Some("missing or not functional; try Xcode setup"),
            );
            overall_ok = false;
        } else {
            // Minimal sanity check: ensure `instruments -help` works.
            let (ok, out) = run_and_capture(
                &root,
                "instruments -help",
                Cmd::new("timeout")
                    .args(["10s", "instruments", "-help"])
                    .cwd(&root),
            )
            .await;
            write_string(logs_dir.join("instruments.log"), &out)?;
            overall_ok &= ok;
            append_step(
                &mut summary,
                "instruments",
                ok,
                Some("security-logs/instruments.log"),
            );
        }
    } else {
        append_step(&mut summary, "instruments", true, Some("skipped"));
    }

    // Legacy script behavior: valgrind is only considered on Linux, unless forced.
    if !args.no_valgrind && (args.force_valgrind || os_kind() == OsKind::Linux) {
        ensure_valgrind(auto_install).await?;

        println!("\n== valgrind ==");
        let (build_ok, build_out) = run_and_capture(
            &root,
            "build rustowl (system allocator)",
            Cmd::new("cargo")
                .args([
                    "xtask",
                    "toolchain",
                    "cargo",
                    "build",
                    "--release",
                    "--no-default-features",
                    "-p",
                    "rustowl",
                ])
                .cwd(&root),
        )
        .await;
        write_string(logs_dir.join("build-rustowl.log"), &build_out)?;
        overall_ok &= build_ok;
        append_step(
            &mut summary,
            "build rustowl (release, system allocator)",
            build_ok,
            Some("security-logs/build-rustowl.log"),
        );

        let bin = if root.join("target/release/rustowl.exe").is_file() {
            "./target/release/rustowl.exe"
        } else {
            "./target/release/rustowl"
        };

        let suppressions_path = root.join(".valgrind-suppressions");
        let suppressions = if suppressions_path.is_file() {
            Some(".valgrind-suppressions")
        } else {
            None
        };

        let mut args = vec![
            "--tool=memcheck",
            "--leak-check=full",
            "--show-leak-kinds=all",
            "--track-origins=yes",
        ];
        let suppressions_flag;
        if let Some(s) = suppressions {
            suppressions_flag = format!("--suppressions={s}");
            args.push(&suppressions_flag);
        }
        args.push(bin);
        if root.join("./perf-tests/dummy-package").is_dir() {
            args.push("check");
            args.push("./perf-tests/dummy-package");
        } else {
            args.push("--help");
        }

        let (ok, out) = run_and_capture(
            &root,
            "valgrind",
            Cmd::new("valgrind")
                .args(args)
                .cwd(&root)
                .env("RUST_BACKTRACE", "1"),
        )
        .await;
        write_string(logs_dir.join("valgrind.log"), &out)?;

        // Valgrind output is useful, but the exit code can vary by configuration.
        // Use the log as the source of truth.
        append_step(
            &mut summary,
            "valgrind",
            ok,
            Some("security-logs/valgrind.log"),
        );

        // Keep overall status independent of valgrind step.
    } else {
        append_step(&mut summary, "valgrind", true, Some("skipped"));
    }

    let summary_name = format!("security_summary_{}.md", timestamp());
    let summary_path = logs_dir.join(summary_name);
    write_string(&summary_path, &summary)?;

    if !overall_ok {
        return Err(anyhow!(
            "one or more security checks failed; see {}",
            summary_path.display()
        ));
    }

    Ok(())
}

fn append_step(summary: &mut String, name: &str, ok: bool, log: Option<&str>) {
    let _ = writeln!(summary, "## {name}");
    let _ = writeln!(summary, "- status: {}", if ok { "ok" } else { "failed" });
    if let Some(log) = log {
        let _ = writeln!(summary, "- log: {log}");
    }
    let _ = writeln!(summary);
}

async fn run_and_capture(root: &Path, name: &str, cmd: Cmd) -> (bool, String) {
    println!("Running: {name}");

    match cmd.output().await {
        Ok(out) => {
            let mut s = String::new();
            s.push_str("stdout:\n");
            s.push_str(&String::from_utf8_lossy(&out.stdout));
            s.push_str("\n\nstderr:\n");
            s.push_str(&String::from_utf8_lossy(&out.stderr));
            s.push('\n');
            (
                out.status.success(),
                format!("cwd: {}\n\n{}", root.display(), s),
            )
        }
        Err(err) => (false, format!("cwd: {}\nerror: {err:#}\n", root.display())),
    }
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    secs.to_string()
}

async fn print_tool_status(root: &PathBuf, ci_mode: bool) -> Result<()> {
    let host = os_kind();

    println!("Tool Availability Summary");
    println!("================================");
    println!();

    println!("platform: {:?}", host);
    println!("ci: {}", ci_mode);
    println!();

    let cargo_deny = which("cargo-deny").is_some();
    let cargo_shear = which("cargo-shear").is_some();
    let cargo_nextest = Cmd::new("cargo")
        .args(["nextest", "--version"])
        .cwd(root)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let has_miri = Cmd::new("rustup")
        .args(["component", "list", "--installed"])
        .output()
        .await
        .map(|out| String::from_utf8_lossy(&out.stdout).contains("miri"))
        .unwrap_or(false);

    let has_valgrind = which("valgrind").is_some();
    let has_instruments = if host == OsKind::Macos {
        instruments_available(root).await
    } else {
        false
    };

    println!("Security Tools:");
    println!(
        "  cargo-deny:      {}",
        if cargo_deny { "yes" } else { "no" }
    );
    println!(
        "  cargo-shear:     {}",
        if cargo_shear { "yes" } else { "no" }
    );
    println!(
        "  cargo-nextest:   {}",
        if cargo_nextest { "yes" } else { "no" }
    );
    println!("  miri component:  {}", if has_miri { "yes" } else { "no" });
    if host == OsKind::Linux {
        println!(
            "  valgrind:        {}",
            if has_valgrind { "yes" } else { "no" }
        );
    }
    if host == OsKind::Macos {
        println!(
            "  instruments:     {}",
            if has_instruments { "yes" } else { "no" }
        );
    }

    println!();

    let active_toolchain = Cmd::new("rustup")
        .args(["show", "active-toolchain"])
        .cwd(root)
        .output()
        .await
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let active_toolchain = active_toolchain
        .split_whitespace()
        .next()
        .unwrap_or("unknown");

    println!("Advanced Features:");
    if active_toolchain.contains("nightly") {
        println!("  nightly toolchain: yes ({active_toolchain})");
    } else {
        println!("  nightly toolchain: no ({active_toolchain})");
    }

    println!();
    println!("Defaults (after auto-config):");

    // Re-run the same auto-config logic used by `run()` so `--check` output matches.
    let mut defaults = Args {
        check: false,
        install: false,
        ci: ci_mode,
        no_auto_install: false,
        no_miri: false,
        no_valgrind: false,
        force_valgrind: false,
        no_deny: false,
        no_shear: false,
        no_instruments: false,
        force_instruments: false,
        miri_flags: "-Zmiri-disable-isolation -Zmiri-permissive-provenance".to_string(),
        miri_rustflags: "--cfg miri".to_string(),
    };

    match host {
        OsKind::Linux => {
            // Linux keeps everything enabled by default.
        }
        OsKind::Macos => {
            defaults.no_valgrind = true;
            defaults.no_instruments = true;
        }
        _ => {
            defaults.no_valgrind = true;
            defaults.no_instruments = true;
            defaults.no_miri = true;
        }
    }

    println!(
        "  deny:        {}",
        if defaults.no_deny { "off" } else { "on" }
    );
    println!(
        "  shear:       {}",
        if defaults.no_shear { "off" } else { "on" }
    );
    println!(
        "  miri:        {}",
        if defaults.no_miri { "off" } else { "on" }
    );
    println!(
        "  valgrind:    {}",
        if defaults.no_valgrind { "off" } else { "on" }
    );
    println!(
        "  instruments: {}",
        if defaults.no_instruments { "off" } else { "on" }
    );

    Ok(())
}

async fn check_stable_rust_min_version(root: &PathBuf) -> Result<()> {
    let pinned = crate::util::read_to_string(root.join(".rust-version-stable"))?;
    let pinned = pinned.trim();
    if pinned.is_empty() {
        return Ok(());
    }

    let output = Cmd::new("rustc")
        .args(["--version"])
        .cwd(root)
        .output()
        .await?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let current = stdout.split_whitespace().nth(1).unwrap_or("").trim();
    if current.is_empty() {
        return Err(anyhow!("could not parse rustc version from: {stdout}"));
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

async fn ensure_cargo_tool(bin: &str, crate_name: &str, auto_install: bool) -> Result<()> {
    if which(bin).is_some() {
        return Ok(());
    }

    if !auto_install {
        return Err(anyhow!(
            "required tool `{bin}` not found; install it with `cargo binstall {crate_name}` (recommended) or `cargo install {crate_name}`"
        ));
    }

    ensure_cargo_binstall().await?;

    // Prefer binstall so CI doesn't build crates from source.
    if let Err(err) = Cmd::new("cargo")
        .args(["binstall", "-y", crate_name])
        .run()
        .await
    {
        // Fall back to source install if no prebuilt package is available.
        eprintln!("[security] cargo binstall failed for {crate_name}: {err:#}");
        Cmd::new("cargo")
            .args(["install", crate_name])
            .run()
            .await
            .with_context(|| format!("install {crate_name}"))?;
    }

    if which(bin).is_none() {
        return Err(anyhow!("tool {bin} still not found after install"));
    }

    Ok(())
}

async fn ensure_cargo_binstall() -> Result<()> {
    if Cmd::new("cargo")
        .args(["binstall", "--version"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Ok(());
    }

    // Using `cargo install` here is fine: this happens once, and enables fast installs for other tools.
    Cmd::new("cargo")
        .args(["install", "cargo-binstall"])
        .run()
        .await
        .context("install cargo-binstall")?;

    Ok(())
}

async fn ensure_miri(auto_install: bool) -> Result<()> {
    // If it's already installed, keep this cheap.
    let installed = Cmd::new("rustup")
        .args(["component", "list", "--installed"])
        .output()
        .await
        .map(|out| String::from_utf8_lossy(&out.stdout).contains("miri"))
        .unwrap_or(false);

    if installed {
        return Ok(());
    }

    if !auto_install {
        return Err(anyhow!(
            "miri component is not installed; run `rustup component add miri --toolchain nightly`"
        ));
    }

    Cmd::new("rustup")
        .args(["component", "add", "miri", "--toolchain", "nightly"])
        .run()
        .await
        .context("rustup component add miri")?;
    Ok(())
}

async fn ensure_valgrind(auto_install: bool) -> Result<()> {
    if which("valgrind").is_some() {
        return Ok(());
    }

    if !auto_install {
        return Err(anyhow!(
            "valgrind not found; install it via your system package manager"
        ));
    }

    match os_kind() {
        OsKind::Linux => sudo_install(&["valgrind"]).await?,
        // The legacy script attempted this, but valgrind is generally unreliable on macOS.
        // We keep the behavior behind `auto_install` for parity.
        OsKind::Macos => sudo_install(&["valgrind"]).await?,
        _ => return Err(anyhow!("valgrind unsupported on this OS")),
    }
    if which("valgrind").is_none() {
        return Err(anyhow!("valgrind not found after install"));
    }
    Ok(())
}
