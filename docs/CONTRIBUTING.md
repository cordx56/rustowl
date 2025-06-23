# Contribution guide

_This document is under construction_.

Thank you for considering to contribute RustOwl!

In this document we describe how to contribute our project, as follows:

- How to setup development environment
- Checklist before submitting PR

## Set up your environment

Here we describe how to set up your development environment.

### Manual Development Setup

Set up your development environment manually by installing the required tools for your platform.

#### Prerequisites

**Common Requirements:**
- Rust toolchain (automatically managed via `rust-toolchain.toml`)
- Basic build tools

**Platform-Specific Tools:**

**Linux:**
```bash
sudo apt-get update
sudo apt-get install -y valgrind bc gnuplot build-essential
```

**macOS:**
```bash
brew install gnuplot
# Optional: brew install valgrind (limited support)
```

**Node.js Development (for VS Code extension):**
- Node.js and yarn for VS Code extension development

### Rust code

In the Rust code, we utilize nightly compiler features, which require some tweaks.
Before starting this section, you might be required to install `rustup` since our project requires nightly compiler.

Our project uses `rust-toolchain.toml` to automatically manage the correct Rust version.

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

### Manual Checks

If the automated scripts are not available, ensure:

#### Rust code correctness and formatting
- Correctly formatted by `cargo fmt`
- Linted using Clippy by `cargo clippy`
- All tests pass with `cargo test`

#### VS Code extension Style
- Correctly formatted by `yarn prettier --write src`
- Linting passes with `yarn lint`
- Type checking passes with `yarn check-types`

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
   # Run comprehensive validation
   ./scripts/dev-checks.sh
   ./scripts/security.sh
   ./scripts/bench.sh --load before-changes
   ./scripts/size-check.sh
   ```

### Integration with CI

Our scripts are designed to match CI workflows:
- **`security.sh`** ↔ **`.github/workflows/security.yml`**
- **`bench.sh`** ↔ **`.github/workflows/bench-performance.yml`**
- **`dev-checks.sh`** ↔ **`.github/workflows/checks.yml`**

This ensures local testing provides the same results as CI.

## Troubleshooting

### Script Permissions
```bash
chmod +x scripts/*.sh
```

### Missing Tools

Install the required tools manually using the platform-specific commands in the Prerequisites section above.

### Platform-Specific Issues

#### Linux
```bash
sudo apt-get update
sudo apt-get install -y valgrind bc gnuplot build-essential
```

#### macOS
```bash
brew install gnuplot
# Valgrind has limited support on macOS
```

### CI Failures
- Check workflow logs for specific error messages
- Verify `rust-toolchain.toml` compatibility
- Ensure scripts have execution permissions
- Test locally with the same script used in CI

For more detailed information about the scripts, see [`scripts/README.md`](../scripts/README.md).