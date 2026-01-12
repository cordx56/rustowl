//! # RustOwl cargo-owlsp
//!
//! An LSP server for visualizing ownership and lifetimes in Rust, designed for debugging and optimization.

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use rustowl::{
    Backend,
    cli::{Cli, Commands, ToolchainCommands},
    toolchain, utils,
};
use std::env;
use tower_lsp_server::{LspService, Server};
use tracing_subscriber::filter::LevelFilter;

fn log_level_from_args(args: &Cli) -> LevelFilter {
    args.verbosity.tracing_level_filter()
}

#[cfg(all(any(target_os = "linux", target_os = "macos"), not(miri)))]
use tikv_jemallocator::Jemalloc;

// Use jemalloc by default, but fall back to system allocator for Miri
#[cfg(all(any(target_os = "linux", target_os = "macos"), not(miri)))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

/// Handles the execution of RustOwl CLI commands.
///
/// This function processes a specific CLI command and executes the appropriate
/// subcommand. It handles all CLI operations including analysis checking, cache cleaning,
/// toolchain management, and shell completion generation.
///
/// # Arguments
///
/// * `command` - The specific command to execute
///
/// # Returns
///
/// This function may exit the process with appropriate exit codes:
/// - Exit code 0 on successful analysis
/// - Exit code 1 on analysis failure or toolchain setup errors
async fn handle_command(command: Commands, rustc_threads: usize) {
    match command {
        Commands::Check(command_options) => {
            let path = command_options.path.unwrap_or_else(|| {
                env::current_dir().unwrap_or_else(|_| {
                    tracing::error!("Failed to get current directory, using '.'");
                    std::path::PathBuf::from(".")
                })
            });

            let report = Backend::check_report_with_options(
                &path,
                command_options.all_targets,
                command_options.all_features,
                rustc_threads,
            )
            .await;

            if report.ok {
                match report.total_targets {
                    Some(total) => {
                        eprintln!(
                            "rustowl check: success ({}/{}) in {:.2?}",
                            report.checked_targets, total, report.duration
                        );
                    }
                    None => {
                        eprintln!("rustowl check: success in {:.2?}", report.duration);
                    }
                }
                std::process::exit(0);
            }
            tracing::error!("Analyze failed");
            std::process::exit(1);
        }
        Commands::Clean => {
            if let Ok(meta) = cargo_metadata::MetadataCommand::new().exec() {
                let target = meta.target_directory.join("owl");
                tokio::fs::remove_dir_all(&target).await.ok();
            }
        }
        Commands::Toolchain(command_options) => {
            if let Some(arg) = command_options.command {
                match arg {
                    ToolchainCommands::Install {
                        path,
                        skip_rustowl_toolchain,
                    } => {
                        let path = path.unwrap_or(toolchain::FALLBACK_RUNTIME_DIR.clone());
                        if toolchain::setup_toolchain(&path, skip_rustowl_toolchain)
                            .await
                            .is_err()
                        {
                            std::process::exit(1);
                        }
                    }
                    ToolchainCommands::Uninstall => {
                        rustowl::toolchain::uninstall_toolchain().await;
                    }
                }
            }
        }
        Commands::Completions(command_options) => {
            let shell = command_options.shell;
            generate(
                shell,
                &mut Cli::command(),
                "rustowl",
                &mut std::io::stdout(),
            );
        }
    }
}

/// Handles the case when no command is provided (version display or LSP server mode)
async fn handle_no_command(args: Cli, used_short_flag: bool, rustc_threads: usize) {
    if args.version {
        if used_short_flag {
            println!("rustowl {}", clap::crate_version!());
        } else {
            display_version();
        }
        return;
    }

    start_lsp_server(rustc_threads).await;
}

/// Displays version information including git tag, commit hash, build time, etc.
fn display_version() {
    println!("rustowl {}", clap::crate_version!());

    let tag = env!("GIT_TAG");
    println!("tag:{}", if tag.is_empty() { "not found" } else { tag });

    let commit = env!("GIT_COMMIT_HASH");
    println!(
        "commit_hash:{}",
        if commit.is_empty() {
            "not found"
        } else {
            commit
        }
    );

    let build_time = env!("BUILD_TIME");
    println!(
        "build_time:{}",
        if build_time.is_empty() {
            "not found"
        } else {
            build_time
        }
    );

    let rustc_version = env!("RUSTC_VERSION");
    if rustc_version.is_empty() {
        println!("build_env:not found");
    } else {
        println!("build_env:{},{}", rustc_version, env!("RUSTOWL_TOOLCHAIN"));
    }
}

/// Starts the LSP server
async fn start_lsp_server(rustc_threads: usize) {
    eprintln!("RustOwl v{}", clap::crate_version!());
    eprintln!("This is an LSP server. You can use --help flag to show help.");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(Backend::new(rustc_threads))
        .custom_method("rustowl/cursor", Backend::cursor)
        .custom_method("rustowl/analyze", Backend::analyze)
        .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}

#[tokio::main]
async fn main() {
    let used_short_flag = std::env::args().any(|arg| arg == "-V");

    let parsed_args = Cli::parse();
    let rustc_threads = parsed_args
        .rustc_threads
        .unwrap_or(utils::get_default_parallel_count());

    rustowl::initialize_logging(log_level_from_args(&parsed_args));

    match parsed_args.command {
        Some(command) => handle_command(command, rustc_threads).await,
        None => handle_no_command(parsed_args, used_short_flag, rustc_threads).await,
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use rustowl::async_test;

    // Command handling in this binary calls `std::process::exit`, which makes it
    // hard to test directly. Clap parsing is covered in `src/cli.rs`.

    #[test]
    fn test_display_version_function() {
        super::display_version();
    }

    #[test]
    fn log_level_from_args_uses_cli_verbosity() {
        let args = rustowl::cli::Cli::parse_from(["rustowl", "-vv"]);
        let level = super::log_level_from_args(&args);
        assert_eq!(level, args.verbosity.tracing_level_filter());
    }

    async_test!(handle_no_command_prints_version_for_long_flag, async {
        let args = rustowl::cli::Cli::parse_from(["rustowl", "--version"]);

        let output = gag::BufferRedirect::stdout().unwrap();
        super::handle_no_command(args, false, 1).await;

        drop(output);
    });
}
