# Contribution guide

_This document is under construction_.

Thank you for considering to contribute RustOwl!

In this document we describe how to contribute our project, as follows:

- How to setup development environment
- Checklist before submitting PR

## Set up your environment

Here we describe how to set up your development environment.

### Rust code

In the Rust code, we utilize nightly compiler features, which require some tweaks.
Before starting this section, you might be required to install `rustup` since our project requires nightly compiler.

#### Build and test using the nightly environment

For building, testing, or installing, you can do the same as any common Rust project using the `cargo` command.
Here, `cargo` must be a `rustup`-proxied command, which is usually installed with `rustup`.

#### Build with stable Rust compiler

To distribute release binary, we use stable Rust compiler to ship RustOwl with stable Rust compiler for users.

The executable binary named `rustowlc`, which is one of the components of RustOwl, behaves like a Rust compiler.
So we would like to compile `rustowlc`, which uses nightly features, with the stable Rust compiler.

Note: Using this method is strongly discouraged officially. See [Unstable Book](doc.rust-lang.org/nightly/unstable-book/compiler-flags/rustc-bootstrap.html).

To compile `rustowlc` with stable compiler, you should set environment variable as `RUSTC_BOOTSTRAP=1`.

For example building with stable 1.87.0 Rust compiler:

```bash
RUSTC_BOOTSTRAP=1 rustup run 1.87.0 cargo build --release
```

Note that by using normal `cargo` command RustOwl will be built with nightly compiler since there is a `rust-toolchain.toml` which specifies nightly compiler for development environment.

### VS Code extension

For VS Code extension, we use `yarn` to setup environment.
To get started, you have to install dependencies by running following command inside `vscode` directory:

```bash
yarn install
```

## Before submitting PR

Before submitting PR, you have to check below:

### Rust code correctness and formatting

- Correctly formatted by `cargo fmt`
- Linted using Clippy by `cargo clippy`

### VS Code extension Style

- Correctly formatted by `yarn fmt`

### Performance Testing

Use the performance testing script to validate the performance characteristics of your changes:

```bash
./scripts/perf-tests.sh --test
```

The script measures various performance metrics of the current code state, including execution time, memory usage, binary size, symbol counts, and more.

#### Basic Usage

- `./scripts/perf-tests.sh --test` - Run performance measurements (default mode)
- `./scripts/perf-tests.sh --verify` - Check system readiness and tool availability
- `./scripts/perf-tests.sh --help` - See all options

#### Common Testing Scenarios

```bash
# Basic performance test with default metrics (size + time)
./scripts/perf-tests.sh --test

# Comprehensive testing with all available metrics
./scripts/perf-tests.sh --test --full

# Test with comprehensive RustOwl analysis (all targets and features)
./scripts/perf-tests.sh --test --all-targets --all-features

# Memory-focused testing with multiple runs
./scripts/perf-tests.sh --test --memory --symbols --runs 5

# Save results to markdown for documentation
./scripts/perf-tests.sh --test --markdown results.md

# Compare with previous results
./scripts/perf-tests.sh --test --compare previous-results.md

# Benchmark performance impact of comprehensive analysis vs default
./scripts/perf-tests.sh --test --runs 5 --markdown default-analysis.md
./scripts/perf-tests.sh --test --all-targets --all-features --runs 5 --markdown comprehensive-analysis.md
```

#### Metrics Available

- **Binary Analysis**: Size, symbol count, debug symbol overhead
- **Runtime Performance**: Execution time, memory usage, page faults, context switches
- **Advanced Analysis**: Static analysis, memory profiling, CPU profiling recommendations

#### Performance Considerations

System caches can affect measurements. Use `--cold` (default) to clear caches between tests for consistent results, or `--warm` to test with realistic cached performance.

**Requirements:** `git`, `cargo`, `bc`  
**Optional:** `hyperfine`, `valgrind`, `nm`, `perf` for enhanced analysis

See the man page (`man ./docs/perf-tests.1`) for detailed documentation.
