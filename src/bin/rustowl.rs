//! # RustOwl cargo-owlsp
//!
//! An LSP server for visualizing ownership and lifetimes in Rust, designed for debugging and optimization.

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use rustowl::*;
use std::env;
use std::io;
use tower_lsp::{LspService, Server};

use crate::cli::{Cli, Commands, ToolchainCommands};

#[cfg(all(not(target_env = "msvc"), not(miri)))]
use tikv_jemallocator::Jemalloc;

// Use jemalloc by default, but fall back to system allocator for Miri
#[cfg(all(not(target_env = "msvc"), not(miri)))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

fn set_log_level(default: log::LevelFilter) {
    log::set_max_level(
        env::var("RUST_LOG")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default),
    );
}

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
async fn handle_command(command: Commands) {
    match command {
        Commands::Check(command_options) => {
            let path = command_options.path.unwrap_or(env::current_dir().unwrap());

            if Backend::check_with_options(
                &path,
                command_options.all_targets,
                command_options.all_features,
            )
            .await
            {
                log::info!("Successfully analyzed");
                std::process::exit(0);
            }
            log::error!("Analyze failed");
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
                    ToolchainCommands::Install { path } => {
                        let path = path.unwrap_or(toolchain::FALLBACK_RUNTIME_DIR.clone());
                        if toolchain::setup_toolchain(&path).await.is_err() {
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
            set_log_level("off".parse().unwrap());
            let shell = command_options.shell;
            generate(shell, &mut Cli::command(), "rustowl", &mut io::stdout());
        }
    }
}

/// Initializes the logging system with colors and default log level
fn initialize_logging() {
    simple_logger::SimpleLogger::new()
        .with_colors(true)
        .init()
        .unwrap();
    set_log_level("info".parse().unwrap());
}

/// Handles the case when no command is provided (version display or LSP server mode)
async fn handle_no_command(args: Cli) {
    if args.version {
        display_version(args.quiet == 0);
        return;
    }

    start_lsp_server().await;
}

/// Displays the version information
fn display_version(show_prefix: bool) {
    if show_prefix {
        print!("RustOwl ");
    }
    println!("v{}", clap::crate_version!());
}

/// Starts the LSP server
async fn start_lsp_server() {
    set_log_level("warn".parse().unwrap());
    eprintln!("RustOwl v{}", clap::crate_version!());
    eprintln!("This is an LSP server. You can use --help flag to show help.");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(Backend::new)
        .custom_method("rustowl/cursor", Backend::cursor)
        .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}

#[tokio::main]
async fn main() {
    initialize_logging();

    let parsed_args = Cli::parse();

    match parsed_args.command {
        Some(command) => handle_command(command).await,
        None => handle_no_command(parsed_args).await,
    }
}
