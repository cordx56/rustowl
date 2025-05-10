use clap::{ArgAction, Args, Parser, Subcommand, ValueHint};

#[derive(Debug, Parser)]
#[command(author)]
pub struct Cli {
    /// Print version.
    #[arg(short('V'), long)]
    pub version: bool,

    #[arg(short, long, action(ArgAction::Count))]
    pub quiet: u8,

    #[arg(long)]
    pub stdio: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Check(Check),
    Clean,
    Toolchain(ToolchainArgs),

    /// Generate shell completions.
    Completions(Completions),
}

#[derive(Args, Debug)]
pub struct Check {
    #[arg(value_name("path"), value_hint(ValueHint::AnyPath))]
    pub path: Option<std::path::PathBuf>,
}

#[derive(Args, Debug)]
pub struct ToolchainArgs {
    #[command(subcommand)]
    pub command: Option<ToolchainCommands>,
}

#[derive(Debug, Subcommand)]
pub enum ToolchainCommands {
    Install,
    Uninstall,
}

#[derive(Args, Debug)]
pub struct Completions {
    /// The shell to generate completions for.
    #[arg(value_enum)]
    pub shell: crate::shells::Shell,
}
