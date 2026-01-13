use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use jiff::{Unit, Zoned};

use crate::util::{Cmd, format_bytes, percent_change, repo_root, write_string};

const DEFAULT_THRESHOLD_PCT: f64 = 10.0;

#[derive(Parser, Debug)]
#[command(
    about = "Track release binary sizes and regressions",
    long_about = "Builds release binaries (via `xtask toolchain`) and reports their sizes.

Subcommands:
- check (default): print current sizes
- baseline: write `baselines/size_baseline.txt`
- compare: compare current sizes to baseline and fail if over threshold
- clean: remove the baseline file",
    args_conflicts_with_subcommands = false,
    subcommand_precedence_over_arg = false
)]
pub struct Args {
    /// Subcommand to run (defaults to `check`)
    #[command(subcommand)]
    command: Option<Command>,

    /// Fail if size increases beyond this percent (compare mode)
    #[arg(short, long, default_value_t = DEFAULT_THRESHOLD_PCT)]
    threshold: f64,
}

#[derive(Subcommand, Debug, Clone, Copy)]
enum Command {
    /// Print current release binary sizes
    Check(VerbosityArgs),

    /// Write `baselines/size_baseline.txt` from current sizes
    Baseline(VerbosityArgs),

    /// Compare current sizes to the baseline
    Compare(VerbosityArgs),

    /// Remove the baseline file
    Clean,
}

#[derive(Parser, Debug, Clone, Copy)]
struct VerbosityArgs {
    /// Show a more verbose, table-style output
    #[arg(short, long)]
    verbose: bool,
}

pub async fn run(args: Args) -> Result<()> {
    let root = repo_root()?;
    let baseline_path = root.join("baselines/size_baseline.txt");

    match args
        .command
        .unwrap_or(Command::Check(VerbosityArgs { verbose: false }))
    {
        Command::Check(verbosity) => {
            let sizes = ensure_built_and_get_sizes(&root).await?;
            if verbosity.verbose {
                print_size_table(&sizes);
            } else {
                for (name, bytes) in &sizes {
                    println!("{name}: {bytes} ({})", format_bytes(*bytes));
                }
            }
        }
        Command::Baseline(verbosity) => {
            let sizes = ensure_built_and_get_sizes(&root).await?;
            let mut out = String::new();
            out.push_str("# RustOwl Binary Size Baseline\n");
            out.push_str(&format!("# Generated on {}\n", timestamp_utc()));
            out.push_str("# Format: binary_name:size_in_bytes\n");
            for (name, bytes) in &sizes {
                out.push_str(&format!("{name}:{bytes}\n"));
            }
            write_string(&baseline_path, &out)?;
            println!("Wrote baseline: {}", baseline_path.display());
            if verbosity.verbose {
                print_size_table(&sizes);
            }
        }
        Command::Clean => {
            if baseline_path.is_file() {
                std::fs::remove_file(&baseline_path)
                    .with_context(|| format!("remove {}", baseline_path.display()))?;
            }
        }
        Command::Compare(verbosity) => {
            let baseline = read_baseline(&baseline_path)?;
            let current = ensure_built_and_get_sizes(&root).await?;

            let mut failed = false;
            for (name, cur) in &current {
                let Some(base) = baseline.get(name) else {
                    eprintln!("warning: no baseline for {name}");
                    continue;
                };
                let change = percent_change(*base as f64, *cur as f64);
                let diff = *cur as i64 - *base as i64;
                let diff_str = if diff >= 0 {
                    format!("+{}", format_bytes(diff as u64))
                } else {
                    format!("-{}", format_bytes((-diff) as u64))
                };
                match change {
                    None => println!("{name}: baseline 0, current {cur}"),
                    Some(pct) => {
                        println!(
                            "{name}: {} -> {} ({diff_str}, {pct:.1}%)",
                            format_bytes(*base),
                            format_bytes(*cur)
                        );
                        if pct > args.threshold {
                            failed = true;
                        }
                    }
                }
            }

            if failed {
                return Err(anyhow!("binary size regression beyond threshold"));
            }

            if verbosity.verbose {
                print_size_table(&current);
            }
        }
    }

    Ok(())
}

async fn ensure_built_and_get_sizes(root: &PathBuf) -> Result<Vec<(String, u64)>> {
    let bins = [
        ("rustowl".to_string(), root.join("target/release/rustowl")),
        ("rustowlc".to_string(), root.join("target/release/rustowlc")),
    ];

    let need_build = bins.iter().any(|(_, p)| !p.is_file());
    if need_build {
        Cmd::new("cargo")
            .args(["xtask", "toolchain", "cargo", "build", "--release"])
            .cwd(root)
            .run()
            .await
            .context("build release")?;
    }

    bins.into_iter()
        .map(|(name, path)| {
            let size = std::fs::metadata(&path)
                .with_context(|| format!("metadata {}", path.display()))?
                .len();
            Ok((name, size))
        })
        .collect()
}

fn print_size_table(sizes: &[(String, u64)]) {
    println!("\n{:<20} {:>12} {:>12}", "Binary", "Bytes", "Formatted");
    println!("{:<20} {:>12} {:>12}", "------", "-----", "---------");
    for (name, bytes) in sizes {
        println!("{:<20} {:>12} {:>12}", name, bytes, format_bytes(*bytes));
    }
    println!();
}

fn timestamp_utc() -> String {
    Zoned::now()
        .in_tz("UTC")
        .ok()
        .and_then(|z| z.round(Unit::Second).ok())
        .map(|z| z.strftime("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn read_baseline(path: &PathBuf) -> Result<std::collections::HashMap<String, u64>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("read baseline {}", path.display()))?;
    let mut map = std::collections::HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, size)) = line.split_once(':') else {
            continue;
        };
        if let Ok(parsed) = size.trim().parse::<u64>() {
            map.insert(name.trim().to_string(), parsed);
        }
    }
    Ok(map)
}
