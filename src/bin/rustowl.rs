//! # RustOwl cargo-owlsp
//!
//! An LSP server for visualizing ownership and lifetimes in Rust, designed for debugging and optimization.

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use mimalloc::MiMalloc;
use rustowl::*;
use std::env;
use std::io;
use tower_lsp::{LspService, Server};

use crate::cli::{Cli, Commands, ToolchainCommands};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn set_log_level(default: log::LevelFilter) {
    log::set_max_level(
        env::var("RUST_LOG")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default),
    );
}

#[tokio::main]
async fn main() {
    simple_logger::SimpleLogger::new()
        .with_colors(true)
        .init()
        .unwrap();
    set_log_level("info".parse().unwrap());

    let matches = Cli::parse();
    if let Some(arg) = matches.command {
        match arg {
            Commands::Check(matches) => {
                let path = matches.path.unwrap_or(env::current_dir().unwrap());
                if Backend::check(&path).await {
                    log::info!("Successfully analyzed");
                    std::process::exit(0);
                } else {
                    log::error!("Analyze failed");
                    std::process::exit(1);
                }
            }
            Commands::Clean => {
                if let Ok(meta) = cargo_metadata::MetadataCommand::new().exec() {
                    let target = meta.target_directory.join("owl");
                    tokio::fs::remove_dir_all(&target).await.ok();
                }
            }
            Commands::Toolchain(matches) => {
                if let Some(arg) = matches.command {
                    match arg {
                        ToolchainCommands::Install => {
                            if toolchain::check_fallback_dir().is_none()
                                && rustowl::toolchain::setup_toolchain().await.is_err()
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
            Commands::Completions(matches) => {
                set_log_level("off".parse().unwrap());
                let shell = matches.shell;
                generate(shell, &mut Cli::command(), "rustowl", &mut io::stdout());
            }
        }
    } else if matches.version {
        if matches.quiet == 0 {
            print!("RustOwl ");
        }
        println!("v{}", clap::crate_version!());
        return;
    } else {
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
}
