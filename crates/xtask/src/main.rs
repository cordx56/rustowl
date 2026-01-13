use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod commands;
mod util;

#[derive(Parser, Debug)]
#[command(author, version, about = "Project maintenance commands")]
#[command(propagate_version = true)]
#[command(disable_version_flag = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run a command under a pinned Rust sysroot
    Toolchain(commands::toolchain::Args),

    /// Run formatting, linting, build and smoke tests
    DevChecks(commands::dev_checks::Args),

    /// Track release binary sizes and regressions
    SizeCheck(commands::size_check::Args),

    /// Run Neovim-based integration tests
    NvimTests(commands::nvim_tests::Args),

    /// Prepare a release tag and bump versions
    Bump(commands::bump::Args),

    /// Run performance benchmarks and compare baselines
    Bench(commands::bench::Args),

    /// Run security-oriented checks (audit, miri, etc.)
    Security(commands::security::Args),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Toolchain(args) => commands::toolchain::run(args).await,
        Command::DevChecks(args) => commands::dev_checks::run(args).await,
        Command::SizeCheck(args) => commands::size_check::run(args).await,
        Command::NvimTests(args) => commands::nvim_tests::run(args).await,
        Command::Bump(args) => commands::bump::run(args).await,
        Command::Bench(args) => commands::bench::run(args).await,
        Command::Security(args) => commands::security::run(args).await,
    }
    .context("xtask failed")
}
