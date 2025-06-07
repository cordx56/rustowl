# RustOwl Scripts

This directory contains utility scripts for local development and testing that complement the CI workflows.

## Quick Start

```bash
# Setup development environment (installs all tools and dependencies)
./scripts/setup-dev.sh

# Check what's already installed
./scripts/setup-dev.sh --check-only

# Run development checks
./scripts/dev-checks.sh

# Check binary size changes
./scripts/size-check.sh

# Run security and memory safety tests
./scripts/security.sh

# Run performance benchmarks
./scripts/bench.sh
```

## Script Overview

### `setup-dev.sh`
Sets up your development environment by installing necessary tools and dependencies.

**Options:**
- `--check-only`: Only check what's installed, don't install anything
- `--help`: Show usage information

**What it installs:**
- Platform-specific development tools
- Rust components (rustfmt, clippy, etc.)
- Testing tools (valgrind on Linux/macOS, etc.)

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

### 🛡️ `security.sh` *(New)*
Comprehensive security and memory safety testing framework.

**Features:**
- Multi-tool testing (Miri, Sanitizers, Valgrind, Audit)
- Cross-platform support (Linux, macOS, Windows, ARM64)
- Graceful degradation when tools unavailable
- Configurable test categories and timeouts
- Color-coded output with progress indicators

**Usage:**
```bash
# Run all available tests
./scripts/security.sh

# Run specific test categories
./scripts/security.sh --miri-only
./scripts/security.sh --sanitizers-only
./scripts/security.sh --audit-only

# Platform-specific testing
./scripts/security.sh --platform linux --timeout 300
```

### 📊 `bench.sh` *(New)*
Local performance benchmarking with regression detection.

**Features:**
- Criterion benchmark integration
- Baseline creation and comparison
- Automatic test package detection
- Configurable regression thresholds
- HTML report generation
- CI/local consistency

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

## Prerequisites

### Common Requirements
- Rust toolchain (automatically managed via `rust-toolchain.toml`)
- Basic build tools

### Platform-Specific Tools

#### Linux
```bash
sudo apt-get update
sudo apt-get install -y valgrind bc gnuplot build-essential
```

#### macOS
```bash
brew install gnuplot
# Optional: brew install valgrind (limited support)
```

#### Windows
- Visual Studio Build Tools
- Optional: Install gnuplot for enhanced benchmark reports

## Integration with CI

These scripts are designed to match their corresponding CI workflows:

- **`security.sh`** ↔ **`.github/workflows/security.yml`**: Same security analysis tools
- **`bench.sh`** ↔ **`.github/workflows/bench-performance.yml`**: Same benchmarks, same tools
- **`setup-dev.sh`** helps install the same tools used in CI locally

This ensures that local testing provides the same results as CI, making development more predictable and reliable.

### GitHub Actions Integration

The scripts integrate with several workflows:

#### Performance Workflows
- **`setup-baseline.yml`**: Creates initial baseline (runs once, then can be deleted)
- **`create-baseline.yml`**: Updates baselines when main branch changes
- **`bench-performance.yml`**: Compares PR performance against baselines

#### Security Workflows
- **`security.yml`**: Runs comprehensive security testing across platforms

#### Composite Actions
- **`.github/actions/comment-benchmark/`**: Reusable action for PR performance comments

## Development Workflow

### Before Committing
```bash
# Run all development checks
./scripts/dev-checks.sh

# Run security tests
./scripts/security.sh

# Check performance impact
./scripts/bench.sh

# Check binary size impact
./scripts/size-check.sh
```

### Setting Up New Environment
```bash
# Install all development tools
./scripts/setup-dev.sh

# Verify installation
./scripts/setup-dev.sh --check-only
```

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
