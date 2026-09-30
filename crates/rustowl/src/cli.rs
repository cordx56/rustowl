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

    /// Show ownership and lifetime visualization for a variable.
    Show(Show),
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

#[derive(Args, Debug)]
pub struct Show {
    /// The path of the file to analyze (optional).
    /// If specified, the function path is relative to this file.
    /// If not specified, the function path is relative to the crate root.
    #[arg(short, long, value_name("path"), value_hint(ValueHint::FilePath))]
    pub path: Option<std::path::PathBuf>,

    /// The path of the function to analyze (e.g., module::function).
    #[arg(value_name("function_path"))]
    pub function_path: String,

    /// The name of the variable to visualize.
    #[arg(value_name("variable"))]
    pub variable: String,

    /// Check all targets.
    #[arg(long, default_value_t = false)]
    pub all_targets: bool,

    /// Check all features.
    #[arg(long, default_value_t = false)]
    pub all_features: bool,
}
