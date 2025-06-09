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
    Install,

    /// Uninstall the toolchain.
    Uninstall,
}

#[derive(Args, Debug)]
pub struct Completions {
    /// The shell to generate completions for.
    #[arg(value_enum)]
    pub shell: crate::shells::Shell,
}
