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
- Multi-tool testing (Miri, Valgrind, cargo-audit, cargo-machete)
- Cross-platform support (Linux, macOS, ARM64)
- Graceful degradation when tools unavailable
- Configurable test categories and timeouts
- Color-coded output with progress indicators

**Usage:**
```bash
# Run all available tests
./scripts/security.sh

# Run specific test categories
./scripts/security.sh --no-miri
./scripts/security.sh --no-valgrind
./scripts/security.sh --no-audit

# Check available tools and configuration
./scripts/security.sh --check
```

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
