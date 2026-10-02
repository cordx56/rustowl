# RustOwl Scripts

This directory contains utility scripts for local development and testing that complement the CI workflows.

## Quick Start

```bash
# Run development checks
./scripts/dev-checks.sh

# Check binary size changes
./scripts/size-check.sh

# Run security and memory safety tests
./scripts/security.sh

# Run performance benchmarks
./scripts/bench.sh

# Run Neovim Tests
./scripts/run_nvim_tests.sh
```

## Script Overview

### `dev-checks.sh`

Runs comprehensive development checks including formatting, linting, and tests.

**Features:**

- Code formatting checks (`cargo fmt`)
- Linting with Clippy
- Unit tests
- Integration tests
- Documentation tests

### `size-check.sh`

Analyzes binary size changes to detect bloat and track optimization efforts.

**Features:**

- Binary size comparison
- Dependency analysis
- Size regression detection

### 🛡️ `security.sh`

Comprehensive security and memory safety testing framework.

**Features:**

- Multi-tool testing (Miri, Valgrind, ThreadSanitizer, cargo-audit, cargo-machete, Instruments)
- Cross-platform support (Linux, macOS, ARM64)
- Graceful degradation when tools unavailable
- Configurable test categories
- Color-coded output with progress indicators

**Usage:**

```bash
# Run all applicable tests for the current platform
./scripts/security.sh

# Run specific test categories
./scripts/security.sh --no-miri
./scripts/security.sh --no-valgrind
./scripts/security.sh --no-audit

# ThreadSanitizer is opt-in: it instruments every build, so it is slow
./scripts/security.sh --thread-sanitizer

# Check available tools and configuration
./scripts/security.sh --check
```

**Platform notes:**

- On Linux, Instruments is disabled and Valgrind covers memory errors.
- On macOS, Valgrind is disabled and a Time Profiler trace is captured with
  `xcrun xctrace` instead. The trace is written as
  `instruments_output_<timestamp>.trace`, which CI uploads as an artifact when
  the run fails.
- `cargo-machete` runs on both Linux and macOS.
- In CI, missing tools are installed automatically unless you pass
  `--no-auto-install`.

### 📊 `bench.sh`

Local performance benchmarking with regression detection.

**Features:**

- Criterion benchmark integration
- Baseline creation and comparison
- Automatic test package detection
- Configurable regression thresholds
- HTML report generation
- Local development focus

**Usage:**

```bash
# Standard benchmark run
./scripts/bench.sh

# Create and compare baselines
./scripts/bench.sh --save my-baseline
./scripts/bench.sh --load my-baseline --threshold 3%

# Development workflow
./scripts/bench.sh --clean --open --test-package ./examples
```

**Note:** Benchmarks are designed for local development only. CI environments introduce too much variability for reliable performance measurement.

### `run_nvim_tests.sh`

Run tests defined in [`nvim-tests`](../nvim-tests) directory.

**Usage**

```bash
./scripts/run_nvim_tests.sh
```

In uses `[mini.test](https://github.com/echasnovski/mini.test)` plugin to test.

## Prerequisites

### Common Requirements

- Rust toolchain (automatically managed via `rust-toolchain.toml`)
- Basic build tools
- Neovim (Only For The Tests And If You Are Changing Neovim Specific Things)

### Platform-Specific Tools

#### Linux

```bash
sudo apt-get update
sudo apt-get install -y valgrind gnuplot build-essential
```

#### macOS

```bash
brew install gnuplot
# Optional: brew install valgrind (limited support)
```

## Integration with CI

These scripts are designed to complement CI workflows where applicable:

- **`security.sh`** ↔ **`.github/workflows/security.yml`**: Same security analysis tools

Note: Benchmarking (`bench.sh`) is intentionally local-only due to CI environment variability.

### GitHub Actions Integration

The scripts integrate with workflows where appropriate:

#### Security Workflows

- **`security.yml`**: Runs comprehensive security testing across platforms

#### Development Scripts

- **`bench.sh`**: Local-only performance testing (not suitable for CI)
- **`dev-checks.sh`**: Can be used in CI for code quality checks

## Development Workflow

### Before Committing

```bash
# Run all development checks
./scripts/dev-checks.sh

# Run security tests
./scripts/security.sh

# Check performance impact (local only)
./scripts/bench.sh

# Check binary size impact
./scripts/size-check.sh

# If you changed any neovim specific things (like lua files)
./scripts/run_nvim_tests.sh
```

### Setting Up New Environment

For setting up a development environment, ensure you have the platform-specific tools listed in the Prerequisites section above.

## Troubleshooting

### Script Permissions

```bash
chmod +x scripts/*.sh
```

### Missing Tools

Run the setup script or check platform-specific installation commands above.

### CI Failures

- Check workflow logs for specific error messages
- Verify `rust-toolchain.toml` compatibility
- Ensure scripts have execution permissions
- Test locally with the same script used in CI

## Script Architecture

All scripts follow common patterns:

- **Color-coded output** with emoji indicators
- **Progressive enhancement** based on available tools
- **Comprehensive help text** with examples
- **Error handling** with remediation suggestions
- **Cross-platform compatibility** with platform-specific optimizations

### Shared code

`lib/common.sh` holds everything the in-repo scripts have in common: the colour
constants and `log_*` helpers, the NixOS `nix-ld` path setup, `have_cmd` /
`require_cmd`, `print_host_tuple`, the rustc version gate, and the integer
percentage helpers. Scripts source it as `lib/common.sh` relative to their own
directory:

```sh
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"
```

The library is POSIX `sh` — no `[[ ]]`, no arrays, no `echo -e` — because
`toolchain` sources it and runs under dash and busybox-ash in CI. It also has no
side effects at source time.

`installer` is the one exception. It is piped into `sh` from a URL and so has
to stay self-contained; it keeps its own copy of `print_host_tuple`.
