use maneuver::prelude::*;

/// Short description, shown in the `NAME` section.
const ABOUT: &str = "visualize Rust ownership and lifetimes";

/// Long description, shown in the `DESCRIPTION` section.
const DESCRIPTION: &str = "\
RustOwl visualizes ownership and lifetimes in Rust programs. It can display
ownership and lifetime visualization for a selected variable.

When no command is provided, RustOwl starts as an LSP server.";

/// Builds the manual page describing the `rustowl` command line interface.
fn manpage() -> Manual {
    let manual = Manual::new(env!("CARGO_PKG_NAME"))
        .about(ABOUT)
        .description(DESCRIPTION)
        .version(env!("CARGO_PKG_VERSION"))
        .source("RustOwl")
        .author(Author::new("Yuki Okamoto and many other contributors"))
        .author(Author::new(
            "Source code: https://github.com/cordx56/rustowl",
        ))
        .command(check_command())
        .command(clean_command())
        .command(toolchain_command())
        .command(toolchain_install_command())
        .command(toolchain_uninstall_command())
        .command(completions_command())
        .command(show_command())
        .flag(
            Flag::new()
                .short("-V")
                .long("--version")
                .help("Print version information."),
        )
        .flag(
            Flag::new()
                .long("--stdio")
                .help("Use stdio to communicate with the LSP server."),
        )
        .flag(
            Flag::new()
                .short("-v")
                .long("--verbose")
                .help("Increase logging verbosity."),
        )
        .flag(
            Flag::new()
                .short("-q")
                .long("--quiet")
                .help("Decrease logging verbosity."),
        );

    let manual = environment().into_iter().fold(manual, Manual::env);
    let manual = exit_statuses().into_iter().fold(manual, Manual::status);
    let manual = examples().into_iter().fold(manual, Manual::example);

    match manpage_date() {
        Some(date) => manual.date(date),
        None => manual,
    }
}

/// The `check` command.
fn check_command() -> Command {
    Command::new("check")
        .help("Check availability for a file or directory.")
        .flag(
            Flag::new()
                .long("--all-targets")
                .help("Run the check for all targets instead of the current one."),
        )
        .flag(
            Flag::new()
                .long("--all-features")
                .help("Run the check for all features instead of the current active ones only."),
        )
        .argument(
            Arg::new("PATH")
                .optional()
                .help("The path of a file or directory to check availability."),
        )
}

/// The `clean` command.
fn clean_command() -> Command {
    Command::new("clean").help("Remove artifacts from the target directory.")
}

/// The `toolchain` command.
fn toolchain_command() -> Command {
    Command::new("toolchain")
        .help("Install or uninstall the toolchain.")
        .argument(
            Arg::new("COMMAND")
                .optional()
                .help("The operation to perform, either install or uninstall."),
        )
}

/// The `toolchain install` command.
fn toolchain_install_command() -> Command {
    Command::new("toolchain install")
        .help("Install the toolchain.")
        .option(
            Opt::new("PATH")
                .long("--path")
                .separator(" ")
                .help("Runtime directory path to install the RustOwl toolchain."),
        )
        .flag(
            Flag::new()
                .long("--skip-rustowl-toolchain")
                .help("Install the Rust toolchain only."),
        )
}

/// The `toolchain uninstall` command.
fn toolchain_uninstall_command() -> Command {
    Command::new("toolchain uninstall").help("Uninstall the toolchain.")
}

/// The `completions` command.
fn completions_command() -> Command {
    Command::new("completions")
        .help("Generate shell completions for the specified shell.")
        .argument(Arg::new("SHELL").help("The shell to generate completions for."))
}

/// The `show` command.
fn show_command() -> Command {
    Command::new("show")
        .help("Show ownership and lifetime visualization for a variable in a function path.")
        .option(
            Opt::new("PATH")
                .short("-p")
                .long("--path")
                .separator(" ")
                .help("The path of the file to analyze. The function path is relative to this file; without it, the function path is relative to the crate root."),
        )
        .flag(Flag::new().long("--all-targets").help("Check all targets."))
        .flag(Flag::new().long("--all-features").help("Check all features."))
        .argument(
            Arg::new("FUNCTION_PATH")
                .help("The path of the function to analyze, e.g. module::function."),
        )
        .argument(Arg::new("VARIABLE").help("The name of the variable to visualize."))
}

/// Environment variables that influence the analysis.
fn environment() -> Vec<Env> {
    vec![
        Env::new("RUSTOWL_CACHE")
            .help("Controls the analysis cache. Set to false or 0 to disable caching."),
        Env::new("RUSTOWL_CACHE_DIR").help("Directory used for the analysis cache."),
        Env::new("RUSTOWLC").help("Path to the rustowlc executable used for analysis."),
        Env::new("RUSTOWLC_WORKSPACE_WRAPPER")
            .help("Path to the compiler wrapper used for Cargo workspace crates."),
        Env::new("RUSTFLAGS").help(
            "Additional Rust compiler flags preserved when constructing the analysis command.",
        ),
        Env::new("CARGO_ENCODED_RUSTFLAGS").help("Rust compiler flags in Cargo's encoded format."),
    ]
}

/// Exit codes reported by `rustowl`.
fn exit_statuses() -> Vec<ExitStatus> {
    vec![
        ExitStatus::new("0", "Successful program execution."),
        ExitStatus::new(
            "1",
            "The analysis failed or the toolchain could not be set up.",
        ),
        ExitStatus::new("101", "The program panicked."),
    ]
}

/// Example invocations.
fn examples() -> Vec<Example> {
    vec![
        Example::new()
            .text("Check availability for the current project")
            .command("rustowl check"),
        Example::new()
            .text("Check availability for all targets and features")
            .command("rustowl check . --all-targets --all-features"),
        Example::new()
            .text("Remove target-directory artifacts")
            .command("rustowl clean"),
        Example::new()
            .text("Install the toolchain with a custom path")
            .command("rustowl toolchain install --path ./runtime"),
        Example::new()
            .text("Install only the standard Rust toolchain")
            .command("rustowl toolchain install --skip-rustowl-toolchain"),
        Example::new()
            .text("Generate shell completions for fish")
            .command("rustowl completions fish"),
        Example::new()
            .text("Visualize a variable in a specific function path")
            .command("rustowl show module::function variable"),
    ]
}
