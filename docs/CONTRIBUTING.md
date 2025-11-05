# Contribution guide

_This document is under construction_.

Thank you for considering to contribute RustOwl!

In this document we describe how to contribute our project, as follows:

- How to setup development environment
- Checklist before submitting PR

## Table of Contents

- [Set up your environment](#set-up-your-environment)
  - [Prerequisites](#prerequisites)
    - [Common Requirements](#common-requirements)
    - [Platform-Specific Tools](#platform-specific-tools)
  - [Rust code](#rust-code)
  - [Editor extensions](#editor-extensions)
- [Before submitting PR](#before-submitting-pr)
  - [Development Checks](#development-checks)
  - [Security and Memory Safety Testing](#security-and-memory-safety-testing)
  - [Performance Testing](#performance-testing)
  - [Binary Size Monitoring](#binary-size-monitoring)
  - [Manual Checks](#manual-checks)
- [Development Workflow](#development-workflow)
- [Troubleshooting](#troubleshooting)

## Set up your environment

### Prerequisites

#### Common Requirements

- Rust toolchain (automatically managed via `rust-toolchain.toml`)
- Basic build tools

#### Platform-Specific Tools

##### Linux

```bash
sudo apt-get update
sudo apt-get install -y valgrind bc gnuplot build-essential # For Rustowl Itself
sudo apt-get install -y neovim # Optional: Install Neovim for neovim plugin development
sudo apt-get install -y emacs # Optional: Install Emacs for emacs plugin development
# Install Visual Studio Code as in their documentation
```

##### macOS

```bash
brew install gnuplot
# Optional: brew install valgrind (limited support)
brew install neovim # Optional: Install Neovim for neovim plugin development
brew install emacs # Optional: Install Emacs for emacs plugin development
# Install Visual Studio Code as in their documentation
```

### Rust code

In the Rust code, we utilize nightly compiler features, which require some tweaks.
Before starting this section, you might be required to install `rustup` since our project requires nightly compiler.

Our project uses `rust-toolchain.toml` to automatically manage the correct Rust version.

#### Build and test using the nightly environment

For building, testing, or installing, you can do the same as any common Rust project using the `cargo` command.

#### Build with stable Rust compiler

To distribute release binary, we use stable Rust compiler to ship RustOwl with stable Rust compiler for users.

The executable binary named `rustowlc`, which is one of the components of RustOwl, behaves like a Rust compiler.
So we would like to compile `rustowlc`, which uses nightly features, with the stable Rust compiler.

> [!NOTE]
> Using this method is strongly discouraged officially. See [Unstable Book](doc.rust-lang.org/nightly/unstable-book/compiler-flags/rustc-bootstrap.html).

To compile `rustowlc` with stable compiler, you should set environment variable as `RUSTC_BOOTSTRAP=1`.

Our script automates most of the work, so to build with the toolchain specified in [channel](../scripts/build/channel) file:

```bash
./scripts/build/toolchain cargo build --release
```

To do it manually:

```bash
# 1.89.0 can be any version
RUSTC_BOOTSTRAP=1 rustup +1.89.0 run cargo build --release
```

As new Rust versions release, their APIs change. Thus, RustOwl code also changes. You might see errors if the version is newer. We strive to make the code in the main branch compatible with the latest Rust version, which is specified in the [channel](../scripts/build/channel) file.

Note that by using normal `cargo` command RustOwl will be built with nightly compiler since there is a `rust-toolchain.toml` which specifies nightly compiler for development environment.

### VS Code extension

For VS Code extension, we use `pnpm` to setup environment.
To get started, you have to install dependencies by running following command inside `vscode` directory:

```bash
pnpm install
```

### Neovim Plugin

You need to install [stylua](https://github.com/JohnnyMorganz/StyLua) and [selene](https://github.com/Kampfkarren/selene) for code formatting and linting.

Now write your code, see [lua](../lua), [ftplugin](../ftplugin), [nvim-tests](../nvim-tests).

Please write a test using [mini.test](https://github.com/echasnovski/mini.test) before submitting a pr, you can run tests using [run_nvim_tests.sh](../scripts/run_nvim_tests.sh).

### Emacs Plugin

<!-- TODO Complete this after @MuntasirSZN pr merges. -->

## Before submitting PR

Before submitting PR, you have to check below:

### Development Checks

Use the helper scripts in `./scripts` to run development checks and fixes. Examples:

```bash
# Run all development checks
./scripts/dev-checks.sh

# Run checks and automatically fix issues where possible
./scripts/dev-checks.sh --fix
```

Refer to individual scripts for details (security, benchmarking, size checks). For CI-consistent runs, prefer the scripts above instead of invoking tools manually.

### Security and Memory Safety Testing

Use `./scripts/security.sh` to run the available security and UB detection tools. See the script for options to skip Miri/Valgrind/audit steps.

### Performance Testing

Use `./scripts/bench.sh` to run performance benchmarks. Create and compare baselines using the script's flags.

### Binary Size Monitoring

Use `./scripts/size-check.sh` to analyze and compare binary sizes. Save/Load baselines via the script flags.

### Manual Checks

If the automated scripts are not available, ensure:

#### Rust code correctness and formatting

- Correctly formatted by `cargo fmt`
- Linted using Clippy by `cargo clippy`
- All tests pass with `cargo test`

#### VS Code extension Style

- Correctly formatted by `pnpm fmt`
- Linting passes with `pnpm lint`
- Type checking passes with `pnpm check-types`

#### Neovim Plugin Checks

- Correctly formatted by `stylua`
- Linting passes with `selene`
- Tests passing with `mini.test`

## Development Workflow

### Recommended Development Process

1. **Before making changes**:

   ```bash
   # Create performance baseline
   ./scripts/bench.sh --save before-changes
   ```

2. **During development**:

   ```bash
   # Run quick checks frequently
   ./scripts/dev-checks.sh --fix
   ```

3. **Before committing**:

   ```bash
   # For Rust Code
   ./scripts/dev-checks.sh
   ./scripts/security.sh
   ./scripts/bench.sh --load before-changes
   ./scripts/size-check.sh
   # For Neovim
   ./scripts/run_nvim_tests.sh
   stylua .
   selene .
   ```

   <!-- TODO Add after @MuntasirSZN pr merges -->
   <!-- # For Emacs
   eask script run test
   eask format elisp-autofmt rustowl.el
   eask lint <linter> -->

## Troubleshooting

### Script Permissions

```bash
chmod +x scripts/*.sh
```

### Missing Tools

Install the required tools manually using the platform-specific commands in the Prerequisites section above.

### CI Failures

- Check workflow logs for specific error messages
- Verify `rust-toolchain.toml` compatibility
- Ensure scripts have execution permissions
- Test locally with the same script used in CI

For more detailed information about the scripts, see [`scripts/README.md`](../scripts/README.md).
