use clap::{Args, Parser, Subcommand, ValueHint};

#[derive(Debug, Parser)]
#[command(author, disable_version_flag = true)]
pub struct Cli {
    /// Print version info (-V short, --version detailed).
    #[arg(short = 'V', long = "version")]
    pub version: bool,

    /// Logging verbosity (-v/-vv/-vvv) or quiet (-q/-qq).
    #[command(flatten)]
    pub verbosity: clap_verbosity_flag::Verbosity<clap_verbosity_flag::WarnLevel>,

    /// Use stdio to communicate with the LSP server.
    #[arg(long)]
    pub stdio: bool,

    /// nightly `rustc` supports parallel compilation
    #[arg(
        long,
        value_name("rustc-threads"),
        help = "Specify the rustc thread count during check"
    )]
    pub rustc_threads: Option<usize>,

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

    #[test]
    fn test_cli_default_parsing() {
        let cli = Cli::try_parse_from(["rustowl"]).unwrap();
        assert!(!cli.version);
        assert!(!cli.stdio);
        assert!(cli.command.is_none());
        assert!(cli.rustc_threads.is_none());
    }

    #[test]
    fn test_check_command_with_flags() {
        let cli =
            Cli::try_parse_from(["rustowl", "check", "--all-targets", "--all-features"]).unwrap();
        match cli.command {
            Some(Commands::Check(check)) => {
                assert!(check.all_targets);
                assert!(check.all_features);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_toolchain_install_skip_rustowl() {
        let cli = Cli::try_parse_from([
            "rustowl",
            "toolchain",
            "install",
            "--skip-rustowl-toolchain",
        ])
        .unwrap();

        match cli.command {
            Some(Commands::Toolchain(toolchain)) => match toolchain.command {
                Some(ToolchainCommands::Install {
                    skip_rustowl_toolchain,
                    ..
                }) => assert!(skip_rustowl_toolchain),
                _ => panic!("Expected Install subcommand"),
            },
            _ => panic!("Expected Toolchain command"),
        }
    }

    #[test]
    fn test_completions_command() {
        let cli = Cli::try_parse_from(["rustowl", "completions", "bash"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Completions(_))));
    }

    #[test]
    fn test_invalid_arguments() {
        let args = vec!["rustowl", "invalid"];
        assert!(Cli::try_parse_from(args).is_err());

        let args = vec!["rustowl", "--invalid-flag"];
        assert!(Cli::try_parse_from(args).is_err());
    }
}
