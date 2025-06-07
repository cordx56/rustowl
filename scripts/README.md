# RustOwl Scripts

This directory contains utility scripts for local development and testing that complement the CI workflows.

## Scripts

### `bench.sh` - Performance Benchmarking
- **Purpose**: Run Criterion performance benchmarks locally
- **Matches**: `.github/workflows/bench-performance.yml` CI workflow
- **Features**:
  - Criterion benchmark execution
  - Baseline saving and comparison
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

## Integration with CI

These scripts are designed to match their corresponding CI workflows:

- **`bench.sh`** ↔ **`bench-performance.yml`**: Same benchmarks, same tools
- **`security.sh`** ↔ **`security.yml`**: Same security analysis tools  
- Both use the same Rust version (`1.87.0`) and test targets

This ensures that local testing provides the same results as CI, making development more predictable and reliable.

## Prerequisites

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

## Usage Tips

1. **Before submitting PRs**: Run both scripts to catch issues early
2. **Performance tracking**: Save baselines regularly with `bench.sh --save`
3. **Security verification**: Use `security.sh --check` to verify tool setup
4. **CI debugging**: These scripts replicate CI behavior for local debugging
