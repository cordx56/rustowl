use anyhow::{Context, Result, anyhow};
use clap::Parser;
use open;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::{collections::BTreeMap, path::PathBuf};

use crate::util::{Cmd, percent_change, repo_root, write_string};

#[derive(Parser, Debug)]
#[command(
    about = "Run divan benches and track performance baselines",
    long_about = "Runs `cargo bench -p rustowl` under the pinned toolchain wrapper.

Modes:
- default: run benchmarks and report parsed results
- `--save <NAME>`: save results to `baselines/performance/<NAME>/`
- `--load <NAME>`: compare against a saved baseline and fail on regressions

Options:
- `--bench <NAME>`: restrict which benches run (repeatable)
- `--clean`: `cargo clean` before benchmarking
- `--quiet`: pass `--quiet` to `cargo bench`
- `--open`: open the generated summary report"
)]
pub struct Args {
    /// Save current benchmark results as baseline (directory name)
    #[arg(long, value_name = "NAME")]
    save: Option<String>,

    /// Load baseline and compare current results against it
    #[arg(long, value_name = "NAME")]
    load: Option<String>,

    /// Regression threshold percent (e.g. 5)
    #[arg(long, default_value_t = 5.0, value_name = "PERCENT")]
    threshold: f64,

    /// Clean build artifacts before benchmarking
    #[arg(long)]
    clean: bool,

    /// Repeat `--bench <NAME>` to restrict benches
    #[arg(long = "bench", value_name = "NAME")]
    benches: Vec<String>,

    /// Emit less output; intended for CI
    #[arg(long)]
    quiet: bool,

    /// Open the generated benchmark summary report
    #[arg(long)]
    open: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaselineFile {
    meta: Meta,
    benches: BTreeMap<String, f64>,
    analysis_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Meta {
    git_sha: Option<String>,
    host: Option<String>,
    rustc: Option<String>,
}

pub async fn run(args: Args) -> Result<()> {
    let root = repo_root()?;

    if args.save.is_some() && args.load.is_some() {
        return Err(anyhow!("--save and --load are mutually exclusive"));
    }

    if args.clean {
        Cmd::new("cargo").args(["clean"]).cwd(&root).run().await?;
    }

    // Run divan benches via cargo bench.
    let mut cmd = Cmd::new("cargo").args(["xtask", "toolchain", "cargo", "bench", "-p", "rustowl"]);
    if !args.benches.is_empty() {
        for b in &args.benches {
            cmd = cmd.args(["--bench", b]);
        }
    } else {
        cmd = cmd.args(["--benches"]);
    }

    if args.quiet {
        cmd = cmd.arg("--quiet");
    }

    let output = cmd.cwd(&root).output().await.context("run cargo bench")?;
    let out_str = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // When parsing fails, capturing raw output is crucial for diagnosing format changes.
    if args.save.is_none() && args.load.is_none() && !args.quiet {
        write_string(root.join("target/xtask/bench_last.log"), &out_str).ok();
    }

    if !output.status.success() {
        return Err(anyhow!("bench command failed"));
    }

    let parsed = parse_divan_output(&out_str).context("parse divan output")?;

    // The legacy script timed `./target/release/rustowl check <test-package>`.
    // That measurement is far noisier than microbench timings and caused flaky regressions.
    // For the Divan migration, we record it as metadata only by default.
    let analysis_time = None;

    let baseline_dir = root.join("baselines/performance");

    if let Some(name) = args.save {
        let dir = baseline_dir.join(&name);
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;

        write_string(dir.join("bench.log"), &out_str)?;

        let baseline = BaselineFile {
            meta: Meta {
                git_sha: git_rev_parse(&root).await.ok(),
                host: rustc_host().await.ok(),
                rustc: rustc_version().await.ok(),
            },
            benches: parsed,
            analysis_seconds: analysis_time,
        };

        let json = serde_json::to_string_pretty(&baseline).context("serialize baseline")?;
        write_string(dir.join("baseline.json"), &(json + "\n"))?;
        if let Some(secs) = analysis_time {
            write_string(dir.join("analysis_time.txt"), &format!("{secs}\n"))?;
        }

        let summary = build_summary_markdown(&baseline, None, args.threshold);
        let summary_path = dir.join("summary.md");
        write_string(&summary_path, &summary)?;

        if args.open {
            let _ = open::that(&summary_path);
        }

        Ok(())
    } else if let Some(name) = args.load {
        let dir = baseline_dir.join(&name);
        let baseline_path = dir.join("baseline.json");
        let baseline: BaselineFile = serde_json::from_str(
            &std::fs::read_to_string(&baseline_path)
                .with_context(|| format!("read {}", baseline_path.display()))?,
        )
        .context("parse baseline")?;

        let cmp = compare(&baseline, &parsed, analysis_time, args.threshold)?;
        let summary = build_summary_markdown(&baseline, Some(&cmp), args.threshold);
        let summary_path = dir.join("summary.md");
        write_string(&summary_path, &summary)?;

        if args.open {
            let _ = open::that(&summary_path);
        }

        Ok(())
    } else {
        // Strict mode: parse and report.
        println!("Parsed {} benches.", parsed.len());

        let cur = BaselineFile {
            meta: Meta {
                git_sha: git_rev_parse(&root).await.ok(),
                host: rustc_host().await.ok(),
                rustc: rustc_version().await.ok(),
            },
            benches: parsed,
            analysis_seconds: None,
        };

        let summary_path = root.join("target/xtask/bench_summary.md");
        std::fs::create_dir_all(summary_path.parent().unwrap())
            .with_context(|| format!("create {}", summary_path.parent().unwrap().display()))?;
        write_string(
            &summary_path,
            &build_summary_markdown(&cur, None, args.threshold),
        )?;

        if args.open {
            let _ = open::that(&summary_path);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct CompareResult {
    benches: Vec<BenchCompare>,
    analysis: Option<BenchCompare>,
    failed: bool,
}

#[derive(Debug, Clone)]
struct BenchCompare {
    name: String,
    baseline: f64,
    current: f64,
    change_pct: Option<f64>,
}

fn compare(
    baseline: &BaselineFile,
    current: &BTreeMap<String, f64>,
    analysis_time: Option<f64>,
    threshold: f64,
) -> Result<CompareResult> {
    let mut failed = false;
    let mut bench_rows = Vec::new();

    for (name, base) in &baseline.benches {
        let Some(cur) = current.get(name) else {
            return Err(anyhow!("missing benchmark in current run: {name}"));
        };

        let change = percent_change(*base, *cur);
        if let Some(pct) = change {
            println!("{name}: {base:.6} -> {cur:.6} ({pct:.2}%)");
            if pct > threshold {
                failed = true;
            }
        }

        bench_rows.push(BenchCompare {
            name: name.to_string(),
            baseline: *base,
            current: *cur,
            change_pct: change,
        });
    }

    let mut analysis_row = None;
    if let (Some(base_analysis), Some(cur_analysis)) = (baseline.analysis_seconds, analysis_time) {
        let change = percent_change(base_analysis, cur_analysis);
        if let Some(pct) = change {
            println!("analysis: {base_analysis:.3}s -> {cur_analysis:.3}s ({pct:.2}%)");
            if pct > threshold {
                failed = true;
            }
        } else {
            println!("analysis: baseline {base_analysis:.3}s current {cur_analysis:.3}s");
            if cur_analysis > 0.0 {
                failed = true;
            }
        }

        analysis_row = Some(BenchCompare {
            name: "analysis".to_string(),
            baseline: base_analysis,
            current: cur_analysis,
            change_pct: change,
        });
    }

    let res = CompareResult {
        benches: bench_rows,
        analysis: analysis_row,
        failed,
    };

    if res.failed {
        Err(anyhow!("benchmark regression beyond threshold"))
    } else {
        Ok(res)
    }
}

fn build_summary_markdown(
    current: &BaselineFile,
    compare: Option<&CompareResult>,
    threshold: f64,
) -> String {
    let mut out = String::new();

    let _ = writeln!(&mut out, "# RustOwl Benchmark Summary");
    let _ = writeln!(&mut out);

    if let Some(rustc) = &current.meta.rustc {
        let _ = writeln!(&mut out, "- rustc: {rustc}");
    }
    if let Some(host) = &current.meta.host {
        let _ = writeln!(&mut out, "- host: {host}");
    }
    if let Some(sha) = &current.meta.git_sha {
        let _ = writeln!(&mut out, "- git: {sha}");
    }
    let _ = writeln!(&mut out, "- threshold: {threshold:.2}%");
    let _ = writeln!(&mut out);

    if let Some(cmp) = compare {
        let _ = writeln!(&mut out, "## Comparison");
        let _ = writeln!(
            &mut out,
            "- status: {}",
            if cmp.failed { "failed" } else { "ok" }
        );
        let _ = writeln!(&mut out);

        let _ = writeln!(
            &mut out,
            "| Benchmark | Baseline (s) | Current (s) | Change |"
        );
        let _ = writeln!(&mut out, "|---|---:|---:|---:|");
        for row in &cmp.benches {
            let change = row
                .change_pct
                .map(|v| format!("{v:.2}%"))
                .unwrap_or_else(|| "n/a".to_string());
            let _ = writeln!(
                &mut out,
                "| {} | {:.6} | {:.6} | {} |",
                row.name, row.baseline, row.current, change
            );
        }
        if let Some(row) = &cmp.analysis {
            let change = row
                .change_pct
                .map(|v| format!("{v:.2}%"))
                .unwrap_or_else(|| "n/a".to_string());
            let _ = writeln!(
                &mut out,
                "| {} | {:.6} | {:.6} | {} |",
                row.name, row.baseline, row.current, change
            );
        }

        let _ = writeln!(&mut out);
    }

    let _ = writeln!(&mut out, "## Current Results");
    let _ = writeln!(&mut out, "| Benchmark | Seconds |");
    let _ = writeln!(&mut out, "|---|---:|");
    for (name, secs) in &current.benches {
        let _ = writeln!(&mut out, "| {name} | {secs:.6} |");
    }
    if let Some(secs) = current.analysis_seconds {
        let _ = writeln!(&mut out, "| analysis | {secs:.6} |");
    }

    out
}

fn parse_divan_output(output: &str) -> Result<BTreeMap<String, f64>> {
    // Current divan output for our benches is a table like:
    // "│  ├─ default  6.931 ms │ ... │ mean 7.457 ms │ ..."
    // To keep this robust, we parse any row that contains a benchmark name and a "mean" value.
    // The key becomes "<group>/<benchmark>" (e.g. "rustowl_check/default").
    let re = Regex::new(
        r"^\s*[│|]\s*[├╰]─\s*(?P<name>[A-Za-z0-9_\-]+)\s+(?P<fast>[0-9]+(?:\.[0-9]+)?)\s*(?P<fast_unit>ns|µs|us|ms|s)\s*[│|]\s*(?P<slow>[0-9]+(?:\.[0-9]+)?)\s*(?P<slow_unit>ns|µs|us|ms|s)\s*[│|]\s*(?P<median>[0-9]+(?:\.[0-9]+)?)\s*(?P<median_unit>ns|µs|us|ms|s)\s*[│|]\s*(?P<mean>[0-9]+(?:\.[0-9]+)?)\s*(?P<mean_unit>ns|µs|us|ms|s)\b",
    )
    .context("compile regex")?;

    fn to_secs(val: f64, unit: &str) -> Option<f64> {
        Some(match unit {
            "ns" => val / 1_000_000_000.0,
            "us" | "µs" => val / 1_000_000.0,
            "ms" => val / 1_000.0,
            "s" => val,
            _ => return None,
        })
    }

    let mut map = BTreeMap::new();
    let mut current_group: Option<String> = None;

    for raw in output.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Group headers look like: "├─ rustowl_check" or "╰─ rustowl_comprehensive".
        if let Some(rest) = trimmed
            .strip_prefix("├─ ")
            .or_else(|| trimmed.strip_prefix("╰─ "))
        {
            current_group = Some(rest.split_whitespace().next().unwrap_or("").to_string());
            continue;
        }
        // Some output lines include the left border '│' before the group marker.
        // Only treat them as group headers if they don't have timing columns.
        if trimmed.matches('│').count() < 2 {
            if let Some(rest) = trimmed
                .strip_prefix("│  ├─ ")
                .or_else(|| trimmed.strip_prefix("│  ╰─ "))
            {
                current_group = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("   ╰─ ") {
                current_group = Some(rest.trim().to_string());
                continue;
            }
        }

        let Some(caps) = re.captures(trimmed) else {
            continue;
        };

        let name = caps.name("name").unwrap().as_str().trim().to_string();
        let mean_val: f64 = caps
            .name("mean")
            .unwrap()
            .as_str()
            .parse()
            .context("parse mean")?;
        let mean_unit = caps.name("mean_unit").unwrap().as_str();
        let Some(secs) = to_secs(mean_val, mean_unit) else {
            continue;
        };

        let key = if let Some(group) = &current_group {
            format!("{group}/{name}")
        } else {
            name
        };

        map.insert(key, secs);
    }

    if map.is_empty() {
        return Err(anyhow!("could not find any divan timing lines"));
    }

    Ok(map)
}

async fn git_rev_parse(root: &PathBuf) -> Result<String> {
    crate::util::ensure_tool("git")?;
    let out = Cmd::new("git")
        .args(["rev-parse", "HEAD"])
        .cwd(root)
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!("git rev-parse failed"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

async fn rustc_version() -> Result<String> {
    let out = Cmd::new("rustc").args(["--version"]).output().await?;
    if !out.status.success() {
        return Err(anyhow!("rustc --version failed"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

async fn rustc_host() -> Result<String> {
    let out = Cmd::new("rustc").args(["-vV"]).output().await?;
    if !out.status.success() {
        return Err(anyhow!("rustc -vV failed"));
    }
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(host) = line.strip_prefix("host: ") {
            return Ok(host.trim().to_string());
        }
    }
    Err(anyhow!("host line not found"))
}
