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

# Create performance baseline
./scripts/bench.sh --save main
```

## Scripts

### `setup-dev.sh` - Development Environment Setup
- **Purpose**: One-command setup of entire development environment
- **Features**:
  - Rust toolchain installation (1.87.0 + nightly)
  - All Rust components (rustfmt, clippy, miri)
  - Cargo tools (cargo-audit, cargo-criterion)
  - Node.js and yarn (for VS Code extension)
  - System tools (bc, valgrind, gnuplot)
  - Directory structure creation
  - Installation validation

```bash
# Examples
./scripts/setup-dev.sh                # Full setup
./scripts/setup-dev.sh --check-only   # Check current status
./scripts/setup-dev.sh --rust-only    # Setup only Rust toolchain
./scripts/setup-dev.sh --skip-node    # Setup everything except Node.js
```

### `bench.sh` - Performance Benchmarking
- **Purpose**: Run Criterion performance benchmarks locally
- **Matches**: `.github/workflows/bench-performance.yml` CI workflow
- **Features**:
  - Criterion benchmark execution
  - Baseline saving and comparison (stored in `baselines/performance/`)
  - Regression detection (>2% by default)
  - HTML report generation
  - Quiet mode for automation

```bash
# Examples
./scripts/bench.sh                    # Run benchmarks
./scripts/bench.sh --save main        # Save as 'main' baseline
./scripts/bench.sh --compare          # Compare with 'main' baseline
./scripts/bench.sh --threshold 5%     # Use 5% regression threshold
```

**Note**: Baselines are stored in `baselines/performance/` and are machine-specific.

### `security.sh` - Security & Memory Safety Testing
- **Purpose**: Run security and memory safety analysis tools
- **Matches**: `.github/workflows/security.yml` CI workflow
- **Features**:
  - Miri (undefined behavior detection)
  - Valgrind (memory error detection, Linux only)
  - Sanitizers (AddressSanitizer, ThreadSanitizer)
  - cargo-audit (security vulnerability scanning)
  - DrMemory (Windows memory debugging)
  - Instruments (macOS performance analysis)

```bash
# Examples
./scripts/security.sh                # Run all available security tests
./scripts/security.sh --check        # Check which tools are available
./scripts/security.sh --no-miri      # Skip Miri tests
```

### `bump.sh` - Version Management
- **Purpose**: Bump version numbers and create releases
- **Features**: Automated version bumping across project files
- **Note**: This script is excluded from automated validation tests

```bash
# Examples
./scripts/bump.sh v0.4.0              # Bump to version 0.4.0
```

### `dev-checks.sh` - Development Checks and Fixes
- **Purpose**: Run local development quality checks with optional auto-fixing
- **Features**:
  - Rust version validation (minimum 1.87)
  - Code formatting (rustfmt)
  - Linting (clippy) with auto-fix support
  - Build verification
  - Unit test detection (currently none in RustOwl)
  - VS Code extension checks (TypeScript/ESLint)

```bash
# Examples
./scripts/dev-checks.sh                # Run all checks
./scripts/dev-checks.sh --fix          # Run checks and auto-fix issues
```

### `size-check.sh` - Binary Size Monitoring
- **Purpose**: Track and validate binary size metrics
- **Features**:
  - Binary size measurement and reporting
  - Baseline creation and comparison (stored in `baselines/size_baseline.txt`)
  - Configurable size increase thresholds
  - Human-readable size formatting

```bash
# Examples
./scripts/size-check.sh                # Check current sizes
./scripts/size-check.sh baseline       # Create size baseline
./scripts/size-check.sh compare        # Compare with baseline
./scripts/size-check.sh -t 15 compare  # Use 15% threshold
```

**Note**: Size baselines are stored in `baselines/` and are machine-specific.

## Integration with CI

These scripts are designed to match their corresponding CI workflows:

- **`bench.sh`** ↔ **`bench-performance.yml`**: Same benchmarks, same tools
- **`security.sh`** ↔ **`security.yml`**: Same security analysis tools  
- Both use the same Rust version (`1.87.0`) and test targets

This ensures that local testing provides the same results as CI, making development more predictable and reliable.

## Prerequisites

Run `./scripts/setup-dev.sh` to install all prerequisites automatically, or install manually:

### Common Requirements
- Rust toolchain 1.87.0: `rustup install 1.87.0`
- Test package: `./perf-tests/dummy-package` (included in repo)

### Performance Benchmarking
- cargo-criterion (optional): `cargo install cargo-criterion`
- gnuplot (optional): For detailed plots
- bc (for regression analysis): `sudo apt-get install bc`

### Security Testing
- Miri: `rustup component add miri`
- cargo-audit: `cargo install cargo-audit`
- Valgrind (Linux): `sudo apt-get install valgrind`
- Nightly toolchain (for sanitizers): `rustup install nightly`

### Development Checks
- All common requirements
- clippy: `rustup component add clippy`
- rustfmt: `rustup component add rustfmt`
- yarn (for VS Code extension): Follow [yarn installation guide](https://yarnpkg.com/getting-started/install)

### Size Monitoring
- bc (for calculations): `sudo apt-get install bc`
- stat command (usually pre-installed on Unix systems)

## Usage Tips

1. **Quick setup**: Run `./scripts/setup-dev.sh` for one-command environment setup
2. **Before submitting PRs**: Run `./scripts/dev-checks.sh --fix` and `./scripts/security.sh`
3. **Performance tracking**: Save baselines regularly with `./scripts/bench.sh --save`
4. **Security verification**: Use `./scripts/security.sh --check` to verify tool setup
5. **CI debugging**: These scripts replicate CI behavior for local debugging
6. **Status checking**: Use `./scripts/setup-dev.sh --check-only` to see what's installed
