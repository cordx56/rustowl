use clap::{ArgAction, Args, Parser, Subcommand, ValueHint};

#[derive(Debug, Parser)]
#[command(author)]
pub struct Cli {
    /// Print version.
    #[arg(short('V'), long)]
    pub version: bool,

    /// Suppress output.
    #[arg(short, long, action(ArgAction::Count))]
    pub quiet: u8,

    /// Use stdio to communicate with the LSP server.
    #[arg(long)]
    pub stdio: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Check availability.
    Check(Check),

    /// Remove artifacts from the target directory.
    Clean,

    /// Install or uninstall the toolchain.
    Toolchain(ToolchainArgs),

    /// Generate shell completions.
    Completions(Completions),
}

#[derive(Args, Debug)]
pub struct Check {
    /// The path of a file or directory to check availability.
    #[arg(value_name("path"), value_hint(ValueHint::AnyPath))]
    pub path: Option<std::path::PathBuf>,

    /// Whether to check for all targets
    /// (default: false).
    #[arg(
        long,
        default_value_t = false,
        value_name("all-targets"),
        help = "Run the check for all targets instead of current only"
    )]
    pub all_targets: bool,

    /// Whether to check for all features
    /// (default: false).
    #[arg(
        long,
        default_value_t = false,
        value_name("all-features"),
        help = "Run the check for all features instead of the current active ones only"
    )]
    pub all_features: bool,
}

#[derive(Args, Debug)]
pub struct ToolchainArgs {
    #[command(subcommand)]
    pub command: Option<ToolchainCommands>,
}

#[derive(Debug, Subcommand)]
pub enum ToolchainCommands {
    /// Install the toolchain.
    Install {
        #[arg(
            long,
            value_name("path"),
            value_hint(ValueHint::AnyPath),
            help = "Runtime directory path to install RustOwl toolchain"
        )]
        path: Option<std::path::PathBuf>,
        #[arg(
            long,
            value_name("skip-rustowl-toolchain"),
            help = "Install Rust toolchain only"
        )]
        skip_rustowl_toolchain: bool,
    },

    /// Uninstall the toolchain.
    Uninstall,
}

#[derive(Args, Debug)]
pub struct Completions {
    /// The shell to generate completions for.
    #[arg(value_enum)]
    pub shell: crate::shells::Shell,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn test_cli_default_parsing() {
        // Test parsing empty args (should work with defaults)
        let args = vec!["rustowl"];
        let cli = Cli::try_parse_from(args).unwrap();

        assert!(!cli.version);
        assert_eq!(cli.quiet, 0);
        assert!(!cli.stdio);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_cli_version_flag() {
        let args = vec!["rustowl", "--version"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.version);

        let args = vec!["rustowl", "-V"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.version);
    }

    #[test]
    fn test_cli_quiet_flags() {
        // Single quiet flag
        let args = vec!["rustowl", "-q"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.quiet, 1);

        // Multiple quiet flags
        let args = vec!["rustowl", "-qq"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.quiet, 2);

        // Long form
        let args = vec!["rustowl", "--quiet"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.quiet, 1);

        // Multiple long form
        let args = vec!["rustowl", "--quiet", "--quiet", "--quiet"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.quiet, 3);
    }

    #[test]
    fn test_cli_stdio_flag() {
        let args = vec!["rustowl", "--stdio"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.stdio);
    }

    #[test]
    fn test_cli_combined_flags() {
        let args = vec!["rustowl", "-V", "--quiet", "--stdio"];
        let cli = Cli::try_parse_from(args).unwrap();

        assert!(cli.version);
        assert_eq!(cli.quiet, 1);
        assert!(cli.stdio);
    }

    #[test]
    fn test_check_command_default() {
        let args = vec!["rustowl", "check"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Check(check)) => {
                assert!(check.path.is_none());
                assert!(!check.all_targets);
                assert!(!check.all_features);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_check_command_with_path() {
        let args = vec!["rustowl", "check", "src/lib.rs"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Check(check)) => {
                assert_eq!(check.path, Some(PathBuf::from("src/lib.rs")));
                assert!(!check.all_targets);
                assert!(!check.all_features);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_check_command_with_flags() {
        let args = vec!["rustowl", "check", "--all-targets", "--all-features"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Check(check)) => {
                assert!(check.path.is_none());
                assert!(check.all_targets);
                assert!(check.all_features);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_check_command_comprehensive() {
        let args = vec![
            "rustowl",
            "check",
            "./target",
            "--all-targets",
            "--all-features",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Check(check)) => {
                assert_eq!(check.path, Some(PathBuf::from("./target")));
                assert!(check.all_targets);
                assert!(check.all_features);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_clean_command() {
        let args = vec!["rustowl", "clean"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Clean) => {}
            _ => panic!("Expected Clean command"),
        }
    }

    #[test]
    fn test_toolchain_command_default() {
        let args = vec!["rustowl", "toolchain"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Toolchain(toolchain)) => {
                assert!(toolchain.command.is_none());
            }
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_toolchain_install_default() {
        let args = vec!["rustowl", "toolchain", "install"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Toolchain(toolchain)) => match toolchain.command {
                Some(ToolchainCommands::Install {
                    path,
                    skip_rustowl_toolchain,
                }) => {
                    assert!(path.is_none());
                    assert!(!skip_rustowl_toolchain);
                }
                _ => panic!("Expected Install subcommand"),
            },
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_toolchain_install_with_path() {
        let args = vec!["rustowl", "toolchain", "install", "--path", "/opt/rustowl"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Toolchain(toolchain)) => match toolchain.command {
                Some(ToolchainCommands::Install {
                    path,
                    skip_rustowl_toolchain,
                }) => {
                    assert_eq!(path, Some(PathBuf::from("/opt/rustowl")));
                    assert!(!skip_rustowl_toolchain);
                }
                _ => panic!("Expected Install subcommand"),
            },
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_toolchain_install_skip_rustowl() {
        let args = vec![
            "rustowl",
            "toolchain",
            "install",
            "--skip-rustowl-toolchain",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Toolchain(toolchain)) => match toolchain.command {
                Some(ToolchainCommands::Install {
                    path,
                    skip_rustowl_toolchain,
                }) => {
                    assert!(path.is_none());
                    assert!(skip_rustowl_toolchain);
                }
                _ => panic!("Expected Install subcommand"),
            },
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_toolchain_install_comprehensive() {
        let args = vec![
            "rustowl",
            "toolchain",
            "install",
            "--path",
            "./local-toolchain",
            "--skip-rustowl-toolchain",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Toolchain(toolchain)) => match toolchain.command {
                Some(ToolchainCommands::Install {
                    path,
                    skip_rustowl_toolchain,
                }) => {
                    assert_eq!(path, Some(PathBuf::from("./local-toolchain")));
                    assert!(skip_rustowl_toolchain);
                }
                _ => panic!("Expected Install subcommand"),
            },
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_toolchain_uninstall() {
        let args = vec!["rustowl", "toolchain", "uninstall"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Toolchain(toolchain)) => match toolchain.command {
                Some(ToolchainCommands::Uninstall) => {}
                _ => panic!("Expected Uninstall subcommand"),
            },
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_completions_command() {
        use crate::shells::Shell;

        let args = vec!["rustowl", "completions", "bash"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Some(Commands::Completions(completions)) => {
                assert_eq!(completions.shell, Shell::Bash);
            }
            _ => panic!("Expected Completions command"),
        }

        // Test with different shells
        let shells = ["bash", "zsh", "fish", "powershell", "elvish", "nushell"];
        for shell in shells {
            let args = vec!["rustowl", "completions", shell];
            let cli = Cli::try_parse_from(args).unwrap();

            match cli.command {
                Some(Commands::Completions(_)) => {}
                _ => panic!("Expected Completions command for shell: {shell}"),
            }
        }
    }

    #[test]
    fn test_invalid_arguments() {
        // Invalid command
        let args = vec!["rustowl", "invalid"];
        assert!(Cli::try_parse_from(args).is_err());

        // Invalid shell for completions
        let args = vec!["rustowl", "completions", "invalid-shell"];
        assert!(Cli::try_parse_from(args).is_err());

        // Invalid flag
        let args = vec!["rustowl", "--invalid-flag"];
        assert!(Cli::try_parse_from(args).is_err());
    }

    #[test]
    fn test_cli_debug_impl() {
        let cli = Cli {
            version: true,
            quiet: 2,
            stdio: true,
            command: Some(Commands::Clean),
        };

        let debug_str = format!("{cli:?}");
        assert!(debug_str.contains("version: true"));
        assert!(debug_str.contains("quiet: 2"));
        assert!(debug_str.contains("stdio: true"));
        assert!(debug_str.contains("Clean"));
    }

    #[test]
    fn test_commands_debug_impl() {
        let check = Commands::Check(Check {
            path: Some(PathBuf::from("test")),
            all_targets: true,
            all_features: false,
        });

        let debug_str = format!("{check:?}");
        assert!(debug_str.contains("Check"));
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("all_targets: true"));
    }

    #[test]
    fn test_complex_cli_scenarios() {
        // Test multiple flags with command
        let args = vec![
            "rustowl",
            "-qqq",
            "--stdio",
            "check",
            "./src",
            "--all-targets",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        assert_eq!(cli.quiet, 3);
        assert!(cli.stdio);
        match cli.command {
            Some(Commands::Check(check)) => {
                assert_eq!(check.path, Some(PathBuf::from("./src")));
                assert!(check.all_targets);
                assert!(!check.all_features);
            }
            _ => panic!("Expected Check command"),
        }
    }
}
