# Contribution guide

_This document is under construction_.

Thank you for considering to contribute RustOwl!

In this document we describe how to contribute our project, as follows:

- How to setup development environment
- Checklist before submitting PR

## Table of Contents

- [Contribution guide](#contribution-guide)
  - [Table of Contents](#table-of-contents)
  - [Set up your environment](#set-up-your-environment)
    - [Prerequisites](#prerequisites)
      - [Common Requirements](#common-requirements)
      - [Platform-Specific Tools](#platform-specific-tools)
        - [Linux](#linux)
        - [macOS](#macos)
    - [Rust code](#rust-code)
      - [Build and test using the nightly environment](#build-and-test-using-the-nightly-environment)
      - [Build with stable Rust compiler](#build-with-stable-rust-compiler)
    - [VS Code extension](#vs-code-extension)
    - [Neovim Plugin](#neovim-plugin)
    - [Emacs Plugin](#emacs-plugin)
  - [Before submitting PR](#before-submitting-pr)
    - [Development Checks](#development-checks)
    - [Security and Memory Safety Testing](#security-and-memory-safety-testing)
    - [Performance Testing](#performance-testing)
    - [Binary Size Monitoring](#binary-size-monitoring)
    - [Manual Checks](#manual-checks)
      - [Rust code correctness and formatting](#rust-code-correctness-and-formatting)
      - [VS Code extension Style](#vs-code-extension-style)
      - [Neovim Plugin Checks](#neovim-plugin-checks)
  - [Development Workflow](#development-workflow)
    - [Recommended Development Process](#recommended-development-process)
  - [Troubleshooting](#troubleshooting)
    - [Script Permissions](#script-permissions)
    - [Missing Tools](#missing-tools)
    - [CI Failures](#ci-failures)

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
sudo apt-get install -y visual-studio-code # Optional: Install VS Code for VS Code extension development
```

##### macOS

```bash
brew install gnuplot
# Optional: brew install valgrind (limited support)
brew install neovim # Optional: Install Neovim for neovim plugin development
brew install emacs # Optional: Install Emacs for emacs plugin development
# TODO add for vscode
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

As new rust version releases, there api's change. Thus, rustowl code also change. You might see errors as version is newer. We strive to make code in main branch compatible with latest rust version, which gets specified in the [channel](../scripts/build/channel) file.

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

<!-- TODO Remove start -->

### Development Checks

We provide a comprehensive development checks script that validates code quality:

```bash
# Run all development checks
./scripts/dev-checks.sh

# Run checks and automatically fix issues where possible
./scripts/dev-checks.sh --fix
```

This script performs:

- Rust version compatibility check
- Code formatting validation (`cargo fmt`)
- Linting with Clippy (`cargo clippy`)
- Build verification
- Unit test execution
- VS Code extension checks (formatting, linting, type checking)

### Security and Memory Safety Testing

Run comprehensive security analysis before submitting:

```bash
# Run all available security tests
./scripts/security.sh

# Check which security tools are available
./scripts/security.sh --check

# Run specific test categories
./scripts/security.sh --no-miri        # Skip Miri tests
./scripts/security.sh --no-valgrind    # Skip Valgrind tests
./scripts/security.sh --no-audit       # Skip cargo-audit
```

The security script includes:

- **Miri**: Undefined behavior detection
- **Valgrind**: Memory error detection (Linux)
- **cargo-audit**: Security vulnerability scanning
- **cargo-machete**: Unused dependency detection (macOS)
- **Platform-specific tools**: Instruments (macOS)

### Performance Testing

Validate that your changes don't introduce performance regressions:

```bash
# Run performance benchmarks
./scripts/bench.sh

# Create a baseline for comparison
./scripts/bench.sh --save my-baseline

# Compare against a baseline with custom threshold
./scripts/bench.sh --load my-baseline --threshold 3%

# Clean build and open HTML report
./scripts/bench.sh --clean --open
```

Performance testing features:

- Criterion benchmark integration
- Baseline creation and comparison
- Configurable regression thresholds (default: 5%)
- Automatic test package detection
- HTML report generation

### Binary Size Monitoring

Check for binary size regressions:

```bash
# Analyze current binary sizes
./scripts/size-check.sh

# Compare against a saved baseline
./scripts/size-check.sh --load previous-baseline

# Save current sizes as baseline
./scripts/size-check.sh --save new-baseline
```

<!-- TODO Remove end

Remove those, those have explanation in scripts/README.md file. Just give a brief overview. -->

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
