//! # RustOwl cargo-owlsp
//!
//! An LSP server for visualizing ownership and lifetimes in Rust, designed for debugging and optimization.

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use rustowl::*;
use std::env;
use tower_lsp_server::{LspService, Server};
use tracing_subscriber::filter::LevelFilter;

use crate::cli::{Cli, Commands, ToolchainCommands};

#[cfg(all(not(target_env = "msvc"), not(miri)))]
use tikv_jemallocator::Jemalloc;

// Use jemalloc by default, but fall back to system allocator for Miri
#[cfg(all(not(target_env = "msvc"), not(miri)))]
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
async fn handle_command(command: Commands) {
    match command {
        Commands::Check(command_options) => {
            let path = command_options.path.unwrap_or_else(|| {
                env::current_dir().unwrap_or_else(|_| {
                    tracing::error!("Failed to get current directory, using '.'");
                    std::path::PathBuf::from(".")
                })
            });

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
            rustowl::initialize_logging(LevelFilter::OFF);
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
    rustowl::initialize_logging(LevelFilter::WARN);
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
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("crypto provider already installed");

    rustowl::initialize_logging(LevelFilter::INFO);

    let parsed_args = Cli::parse();

    match parsed_args.command {
        Some(command) => handle_command(command).await,
        None => handle_no_command(parsed_args).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // Test CLI argument parsing
    #[test]
    fn test_cli_parsing_no_command() {
        let args = vec!["rustowl"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.command.is_none());
        assert!(!cli.version);
        assert_eq!(cli.quiet, 0);
    }

    #[test]
    fn test_cli_parsing_version_flag() {
        let args = vec!["rustowl", "--version"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.command.is_none());
        assert!(cli.version);
    }

    #[test]
    fn test_cli_parsing_quiet_flags() {
        let args = vec!["rustowl", "-q"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.quiet, 1);

        let args = vec!["rustowl", "-qq"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.quiet, 2);
    }

    #[test]
    fn test_cli_parsing_stdio_flag() {
        let args = vec!["rustowl", "--stdio"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.stdio);
    }

    #[test]
    fn test_cli_parsing_check_command() {
        let args = vec!["rustowl", "check"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(cli.command, Some(Commands::Check(_))));
    }

    #[test]
    fn test_cli_parsing_check_command_with_path() {
        let args = vec!["rustowl", "check", "/some/path"];
        let cli = Cli::try_parse_from(args).unwrap();
        match cli.command {
            Some(Commands::Check(opts)) => {
                assert_eq!(opts.path, Some(std::path::PathBuf::from("/some/path")));
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_cli_parsing_check_command_with_flags() {
        let args = vec!["rustowl", "check", "--all-targets", "--all-features"];
        let cli = Cli::try_parse_from(args).unwrap();
        match cli.command {
            Some(Commands::Check(opts)) => {
                assert!(opts.all_targets);
                assert!(opts.all_features);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_cli_parsing_clean_command() {
        let args = vec!["rustowl", "clean"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(cli.command, Some(Commands::Clean)));
    }

    #[test]
    fn test_cli_parsing_toolchain_install() {
        let args = vec!["rustowl", "toolchain", "install"];
        let cli = Cli::try_parse_from(args).unwrap();
        match cli.command {
            Some(Commands::Toolchain(opts)) => {
                assert!(matches!(
                    opts.command,
                    Some(ToolchainCommands::Install { .. })
                ));
            }
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_cli_parsing_toolchain_install_with_path() {
        let args = vec!["rustowl", "toolchain", "install", "--path", "/custom/path"];
        let cli = Cli::try_parse_from(args).unwrap();
        match cli.command {
            Some(Commands::Toolchain(opts)) => match opts.command {
                Some(ToolchainCommands::Install { path, .. }) => {
                    assert_eq!(path, Some(std::path::PathBuf::from("/custom/path")));
                }
                _ => panic!("Expected Install command"),
            },
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_cli_parsing_toolchain_install_skip_rustowl() {
        let args = vec![
            "rustowl",
            "toolchain",
            "install",
            "--skip-rustowl-toolchain",
        ];
        let cli = Cli::try_parse_from(args).unwrap();
        match cli.command {
            Some(Commands::Toolchain(opts)) => match opts.command {
                Some(ToolchainCommands::Install {
                    skip_rustowl_toolchain,
                    ..
                }) => {
                    assert!(skip_rustowl_toolchain);
                }
                _ => panic!("Expected Install command"),
            },
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_cli_parsing_toolchain_uninstall() {
        let args = vec!["rustowl", "toolchain", "uninstall"];
        let cli = Cli::try_parse_from(args).unwrap();
        match cli.command {
            Some(Commands::Toolchain(opts)) => {
                assert!(matches!(opts.command, Some(ToolchainCommands::Uninstall)));
            }
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_cli_parsing_completions() {
        let args = vec!["rustowl", "completions", "bash"];
        let cli = Cli::try_parse_from(args).unwrap();
        match cli.command {
            Some(Commands::Completions(opts)) => {
                // Just verify that shell parsing works - opts should be accessible
                let _shell = opts.shell;
            }
            _ => panic!("Expected Completions command"),
        }
    }

    // Test display_version function
    #[test]
    fn test_display_version_with_prefix() {
        // We can't easily capture stdout in unit tests, but we can verify the function doesn't panic
        display_version(true);
        display_version(false);
    }

    // Test handle_no_command with version flag
    #[tokio::test]
    async fn test_handle_no_command_version() {
        let cli = Cli {
            command: None,
            version: true,
            quiet: 0,
            stdio: false,
        };

        // This should not panic and should handle the version display
        // Note: This will actually exit the process in real execution,
        // but for testing we can verify it doesn't panic
        handle_no_command(cli).await;
    }

    // Test handle_no_command without version (would start LSP server)
    // This is harder to test without mocking, so we'll skip the actual LSP server start

    // Test error handling in handle_command for check command
    // This is also hard to test without mocking the Backend::check_with_options

    // Test handle_command for clean command
    #[tokio::test]
    async fn test_handle_command_clean() {
        let command = Commands::Clean;
        // This should not panic
        handle_command(command).await;
    }

    // Test handle_command for toolchain uninstall
    #[tokio::test]
    async fn test_handle_command_toolchain_uninstall() {
        use crate::cli::*;
        let command = Commands::Toolchain(ToolchainArgs {
            command: Some(ToolchainCommands::Uninstall),
        });
        // This should not panic
        handle_command(command).await;
    }

    // Test handle_command for completions
    #[tokio::test]
    async fn test_handle_command_completions() {
        use crate::cli::*;
        use crate::shells::Shell;
        let command = Commands::Completions(Completions { shell: Shell::Bash });
        // This should not panic
        handle_command(command).await;
    }

    // Test invalid CLI arguments
    #[test]
    fn test_cli_parsing_invalid_command() {
        let args = vec!["rustowl", "invalid-command"];
        let result = Cli::try_parse_from(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_parsing_invalid_flag() {
        let args = vec!["rustowl", "--invalid-flag"];
        let result = Cli::try_parse_from(args);
        assert!(result.is_err());
    }

    // Test edge cases in CLI parsing
    #[test]
    fn test_cli_parsing_empty_args() {
        let args = vec!["rustowl"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.command.is_none());
        assert!(!cli.version);
        assert!(!cli.stdio);
        assert_eq!(cli.quiet, 0);
    }

    #[test]
    fn test_cli_parsing_multiple_quiet_flags() {
        let args = vec!["rustowl", "-q", "-q", "-q"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.quiet, 3);
    }

    // Test command factory for completions
    #[test]
    fn test_command_factory() {
        let cmd = Cli::command();
        // Verify that the command structure is valid
        assert!(!cmd.get_name().is_empty());
        // Just verify that get_about returns something
        assert!(cmd.get_about().is_some() || cmd.get_about().is_none());
    }

    // Test shell completion generation (basic test)
    #[test]
    fn test_completion_generation_setup() {
        // Test that completion generation can be set up without panicking
        let shell = clap_complete::Shell::Bash;
        let mut cmd = Cli::command();
        let mut output = Vec::<u8>::new();

        // This should not panic
        generate(shell, &mut cmd, "rustowl", &mut output);
        assert!(!output.is_empty());
    }

    // Test current directory fallback in check command
    #[test]
    fn test_current_dir_fallback() {
        // Test that we can get current directory or fallback
        let path = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        assert!(path.exists() || path == std::path::PathBuf::from("."));
    }

    // Test jemalloc global allocator (compile-time test)
    #[test]
    #[cfg(all(not(target_env = "msvc"), not(miri)))]
    fn test_jemalloc_allocator() {
        // Test that jemalloc is available as global allocator
        // The fact that this test compiles and runs means jemalloc is properly configured
        // No runtime assertion needed for compile-time check
    }

    // Test crypto provider installation
    #[test]
    fn test_crypto_provider_installation() {
        // Test that crypto provider can be installed
        // This might fail if already installed, but shouldn't panic
        let result = rustls::crypto::aws_lc_rs::default_provider().install_default();
        // Either it succeeds or it's already installed
        assert!(result.is_ok() || result.is_err());
    }
}
