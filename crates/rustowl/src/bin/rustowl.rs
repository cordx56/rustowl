//! # RustOwl cargo-owlsp
//!
//! An LSP server for visualizing ownership and lifetimes in Rust, designed for debugging and optimization.

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use rustowl::*;
use std::env;
use tower_lsp::{LspService, Server};
use tracing_subscriber::filter::LevelFilter;

use crate::cli::{Cli, Commands, ToolchainCommands};

// Cited from rustc
// https://github.com/rust-lang/rust/pull/148925
// MIT License
#[cfg(all(any(target_os = "linux", target_os = "macos"), not(miri)))]
use tikv_jemalloc_sys as _;

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
                tracing::info!("Successfully analyzed");
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
                        skip__toolchain,
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
        Commands::Show(command_options) => {
            handle_show_command(command_options).await;
        }
    }
}

/// Handles the show command for visualizing ownership and lifetimes.
async fn handle_show_command(opts: cli::Show) {
    use rustowl::lsp::analyze::Analyzer;

    // Canonicalize the file path if specified
    let file_path = opts.path.as_ref().and_then(|p| p.canonicalize().ok());

    // Determine the project path for analysis
    let path = file_path
        .clone()
        .unwrap_or_else(|| env::current_dir().unwrap_or(".".into()));

    tracing::info!("Analyzing project at {path:?}");

    // Create an analyzer and run analysis
    let analyzer = match Analyzer::new(&path).await {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("Failed to create analyzer: {e:?}");
            std::process::exit(1);
        }
    };

    let mut iter = analyzer.analyze(opts.all_targets, opts.all_features).await;

    // Collect analysis results
    let mut crate_data: Option<rustowl::models::Crate> = None;
    while let Some(event) = iter.next_event().await {
        match event {
            rustowl::lsp::analyze::AnalyzerEvent::Analyzed(ws) => {
                for krate in ws.0.into_values() {
                    if let Some(existing) = &mut crate_data {
                        existing.merge(krate);
                    } else {
                        crate_data = Some(krate);
                    }
                }
            }
            rustowl::lsp::analyze::AnalyzerEvent::CrateChecked { package, .. } => {
                tracing::debug!("Analyzed: {package}");
            }
        }
    }

    let crate_data = match crate_data {
        Some(data) => data,
        None => {
            tracing::error!("Analysis produced no results");
            std::process::exit(1);
        }
    };

    // Run visualization
    if let Err(e) = rustowl::visualize::show_variable(
        &crate_data,
        file_path.as_deref(),
        &opts.function_path,
        &opts.variable,
    ) {
        tracing::error!("{e}");
        std::process::exit(1);
    }
}

/// Displays detailed version information.
fn display_version() {
    println!("RustOwl {}", clap::crate_version!());

    let tag = env!("GIT_TAG");
    println!("git_tag:{}", if tag.is_empty() { "not found" } else { tag });

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
async fn start_lsp_server() {
    eprintln!("RustOwl v{}", clap::crate_version!());
    eprintln!("This is an LSP server. You can use --help flag to show help.");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(Backend::new)
        .custom_method("rustowl/cursor", Backend::cursor)
        .custom_method("rustowl/analyze", Backend::analyze)
        .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}

#[tokio::main]
async fn main() {
    let short_version = env::args().any(|arg| arg == "-V");

    let parsed_args = Cli::parse();

    let level = match &parsed_args.command {
        Some(Commands::Completions(_)) if !parsed_args.verbosity.is_present() => LevelFilter::OFF,
        _ => log_level_for(&parsed_args.verbosity, parsed_args.command.is_none()),
    };
    initialize_logging(level);

    if parsed_args.version {
        if short_version {
            println!("RustOwl {}", clap::crate_version!());
        } else {
            display_version();
        }
        return;
    }

    match parsed_args.command {
        Some(command) => handle_command(command).await,
        None => start_lsp_server().await,
    }
}
