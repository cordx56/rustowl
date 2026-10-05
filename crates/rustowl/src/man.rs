use man::prelude::*;

fn manpage() -> Manual {
    Manual::new("rustowl")
        .author(Author::new(
            "Maintained by Yuki Okamoto and many other contributors. \n\n\
            Source code: https://github.com/cordx56/rustowl",
         ))
        .about("visualize Rust ownership and lifetimes")
        .description(
            "RustOwl visualizes ownership and lifetimes in Rust programs. \
            It can display ownership and lifetime visualization for a selected variable.\n\n\
            When no command is provided, RustOwl starts as an LSP server.",
        )
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
        )
        .custom(
            Section::new("SUBCOMMANDS")
                .paragraph("check [OPTIONS] [PATH]\n  Check availability for a file or directory.")
                .paragraph("clean\n  Remove artifacts from the target directory.")
                .paragraph("toolchain [COMMAND]\n  Install or uninstall the toolchain.")
                .paragraph("toolchain install [--path <PATH>] [--skip-rustowl-toolchain]\n  Install the toolchain. --skip-rustowl-toolchain installs the Rust toolchain only.")
                .paragraph("toolchain uninstall\n  Uninstall the toolchain.")
                .paragraph("completions <SHELL>\n  Generate shell completions for the specified shell.")
                .paragraph("show [OPTIONS] <FUNCTION_PATH> <VARIABLE>\n  Show ownership and lifetime visualization for a variable in a function path."),
        )
        .option(
            Opt::new("PATH")
                .short("-p")
                .long("--path")
                .help("Optional source file path for check or show commands."),
        )
        .custom(
            Section::new("CHECK OPTIONS")
                .paragraph("--all-targets\n  Run the check for all targets instead of current only (default: false).")
                .paragraph("--all-features\n  Run the check for all features instead of the current active ones only (default: false)."),
        )
        .custom(
            Section::new("SHOW OPTIONS")
                .paragraph("--all-targets\n  Check all targets.")
                .paragraph("--all-features\n  Check all features."),
        )
        // Environment Variables Section
        .custom(
            Section::new("ENVIRONMENT")
                .paragraph("RUSTOWL_CACHE\n  Controls the analysis cache. Set to false or 0 to disable caching.")
                .paragraph("RUSTOWL_CACHE_DIR\n  Directory used for the analysis cache.")
                .paragraph("RUSTOWL_TOOLCHAIN_DIR\n  Runtime directory for the installed RustOwl toolchain.")
                .paragraph("RUSTOWLC\n  Path to the rustowlc executable used for analysis.")
                .paragraph("RUSTOWLC_WORKSPACE_WRAPPER\n  Path to the compiler wrapper used for Cargo workspace crates.")
                .paragraph("RUSTFLAGS\n  Additional Rust compiler flags preserved when constructing the analysis command.")
                .paragraph("CARGO_ENCODED_RUSTFLAGS\n  Rust compiler flags in Cargo's encoded format.")
        )
        .example(
            Example::new()
                .text("Check availability for the current project")
                .command("rustowl check"),
        )
        .example(
            Example::new()
                .text("Check availability for all targets and features")
                .command("rustowl check . --all-targets --all-features"),
        )
        .example(
            Example::new()
                .text("Remove target-directory artifacts")
                .command("rustowl clean"),
        )
        .example(
            Example::new()
                .text("Install the toolchain with a custom path")
                .command("rustowl toolchain install --path ./runtime"),
        )
        .example(
            Example::new()
                .text("Install only the standard Rust toolchain")
                .command("rustowl toolchain install --skip-rustowl-toolchain"),
        )
        .example(
            Example::new()
                .text("Generate shell completions for fish")
                .command("rustowl completions fish"),
        )
        .example(
            Example::new()
                .text("Visualize a variable in a specific function path")
                .command("rustowl show module::function variable"),
        )
}
