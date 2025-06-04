# perf-tests.sh

**Performance testing and analysis for RustOwl**

## NAME

`perf-tests.sh` - Performance testing and analysis for RustOwl

## SYNOPSIS

```bash
perf-tests.sh [OPTION]...
```

## DESCRIPTION

The `perf-tests.sh` script provides comprehensive performance testing and analysis for RustOwl. It measures various performance metrics of the current code state, including binary size, execution time, memory usage, symbol counts, and advanced analysis features.

The script is designed to help developers understand and validate the performance characteristics of their code with flexible metric selection, configurable test runs, and multiple output formats.

## OPTIONS

### Modes

- `-h, --help` - Display help information and exit
- `--verify` - Check the testing environment without running performance tests. Shows system information, tool availability, and enabled metrics.
- `--prepare` - Build the project without running tests. Ensures the binary is ready for testing.
- `--test` - Run performance tests (default mode). Measures selected metrics and displays results.

### Test Configuration

- `--runs N` - Number of test runs to perform (default: 3). Results are averaged across multiple runs for better accuracy.
- `--warm` - Skip cache clearing between runs. Tests performance with system caches intact.
- `--cold` - Clear caches between runs (default). Provides consistent measurements by eliminating cache effects.
- `--force` - Force rebuild even if binary exists. Ensures fresh compilation.

### RustOwl Check Options

- `--all-targets` - Pass `--all-targets` to rustowl check command. Enables analysis of all targets (bins, examples, tests, benches) instead of just the current target.
- `--all-features` - Pass `--all-features` to rustowl check command. Enables analysis with all available features instead of just the default features.

### Metrics Selection

By default, only size and time metrics are enabled. Specifying any metric flag disables defaults and enables only the requested metrics.

- `--full` - Enable all available metrics
- `--size` - Binary size analysis
- `--time` - Execution time measurement
- `--symbols` - Symbol count analysis (requires nm)
- `--strip` - Strip analysis showing debug symbol overhead
- `--memory` - Memory usage measurement
- `--page-faults` - Page fault counting
- `--context-switches` - Context switch measurement
- `--static` - Static analysis metrics
- `--profile-memory` - Memory profiling with valgrind (Linux only)
- `--profile-cpu` - CPU profiling recommendations

### Output Options

- `--markdown FILE` - Save results to markdown file instead of stdout
- `--compare FILE` - Compare current results with previous markdown results file

## TESTING MODES

### Default Mode (--test)

When run without mode arguments, performs performance testing:

1. Builds the project using specific Rust toolchain
2. Runs performance measurements for selected metrics
3. Averages results across multiple runs
4. Displays summary and optionally saves to markdown

### System Verification (--verify)

Check environment readiness:

1. Verify required tools are available
2. Show optional tool availability
3. Display enabled metrics configuration

## MEASUREMENTS

The script provides comprehensive analysis across multiple dimensions:

### Binary Analysis

- Binary size measurement with human-readable formatting
- Symbol count analysis (requires `nm`)
- Strip analysis showing debug symbol overhead
- Static analysis including binary entropy and dependencies

### Performance Metrics

- Execution time (wall clock)
- Peak memory usage (RSS)
- Page faults (major/minor)
- Context switches (voluntary/involuntary)

### Advanced Analysis

When optional tools are available:

- Statistical benchmarking with confidence intervals (`hyperfine`)
- Memory profiling with heap analysis (`valgrind` on Linux)
- CPU profiling recommendations for platform-specific tools

## BUILD SYSTEM

The script uses a specific Rust build configuration:

### Build Command

```bash
RUSTC_BOOTSTRAP=1 rustup run 1.87.0 cargo build --release
```

This ensures consistent compilation with Rust 1.87.0 stable compiler while enabling nightly features through RUSTC_BOOTSTRAP.

### Binary Location

Tests are performed directly on the binary in `./target/release/rustowl` without copying or moving files.

### Test Command

The script executes different rustowl check commands based on the selected options:

- **Default**: `rustowl check ./perf-tests/dummy-package`
- **All Targets**: `rustowl check --all-targets ./perf-tests/dummy-package`
- **All Features**: `rustowl check --all-features ./perf-tests/dummy-package`
- **Comprehensive**: `rustowl check --all-targets --all-features ./perf-tests/dummy-package`

This allows for performance comparison between different analysis modes.

## CACHE MANAGEMENT

Performance measurements can be significantly affected by various system caches. The script provides cache management options:

### Cache Types

- Filesystem caches (page cache) - Most significant impact
- Rust incremental compilation cache
- CPU caches (L1/L2/L3 and branch prediction)

### Cache Clearing Methods

- macOS: `purge` command (may require sudo)
- Linux: `/proc/sys/vm/drop_caches` (requires sudo)
- Rust: `cargo clean` for incremental cache

Use `--cold` (default) for consistent measurements or `--warm` for realistic cached performance.

## RUSTOWL ANALYSIS MODES

The script supports testing different RustOwl analysis modes to measure performance impact:

### Default Analysis Mode

By default, RustOwl performs a fast analysis with standard settings:
- Analyzes only the current target (typically the main binary)
- Uses only default features enabled in Cargo.toml

### Comprehensive Analysis Mode

Use `--all-targets` and/or `--all-features` for thorough analysis:

- `--all-targets`: Analyzes all targets including binaries, examples, tests, and benchmarks
- `--all-features`: Analyzes code with all available features enabled, not just defaults

### Performance Comparison

These modes allow benchmarking the performance trade-offs:

```bash
# Fast analysis (default)
perf-tests.sh --test --runs 5 --markdown fast-analysis.md

# Comprehensive analysis
perf-tests.sh --test --all-targets --all-features --runs 5 --markdown thorough-analysis.md
```

Comprehensive analysis typically takes longer but provides more complete coverage of potential issues.

## DEPENDENCIES

### Required Tools

- `git` - Version control operations
- `cargo` - Rust build system
- `bc` - Mathematical calculations
- `rustup` - Rust toolchain management

### Optional Tools (Enhanced Features)

- `hyperfine` - Statistical benchmarking
- `valgrind` - Memory profiling (Linux only)
- `perf` - CPU profiling (Linux only)
- `nm` - Symbol analysis
- `instruments` - CPU profiling (macOS with Xcode)

## EXAMPLES

### Basic Testing

```bash
# Run basic performance test with default metrics (size + time)
perf-tests.sh --test

# Check system readiness and tool availability
perf-tests.sh --verify

# Run comprehensive test with all available metrics
perf-tests.sh --test --full
```

### Focused Testing

```bash
# Test memory usage and symbol counts with 5 runs for accuracy
perf-tests.sh --test --memory --symbols --runs 5

# Test execution time with warm caches (realistic performance)
perf-tests.sh --test --warm --time

# Test with comprehensive RustOwl analysis (all targets and features)
perf-tests.sh --test --all-targets --all-features

# Compare performance of default vs comprehensive analysis
perf-tests.sh --test --runs 3 --markdown default.md
perf-tests.sh --test --all-targets --all-features --runs 3 --markdown comprehensive.md
```

### Output and Comparison

```bash
# Save results to markdown file for documentation
perf-tests.sh --test --markdown results.md

# Compare current performance with previous results
perf-tests.sh --test --compare baseline.md
```

## RESULT INTERPRETATION

### Normal Results

- Consistent results across multiple runs
- Reasonable execution times and memory usage
- Expected symbol counts for binary size

### Investigate Further

- Extreme performance variations between runs
- Unusually high memory usage or execution time
- Zero page faults (suggests measurement issues)

### Best Practices

- Run `--verify` first to check environment
- Use multiple runs for important measurements
- Test on quiet systems with minimal background processes
- Consider both cold and warm cache performance
- Save results to markdown for tracking over time
- Test both default and comprehensive analysis modes to understand performance trade-offs
- Use `--all-targets --all-features` to benchmark worst-case analysis performance
- Compare analysis modes with consistent run counts for accurate benchmarking

## OUTPUT FORMATS

### Standard Output

Default format displays metrics to stdout with colored output and progress indicators.

### Markdown Output

Use `--markdown FILE` to generate structured markdown reports suitable for documentation and tracking.

### Comparison Output

Use `--compare FILE` to compare current results with previously saved markdown results, showing differences and changes.

## EXIT STATUS

- **0** - Success
- **1** - General error (missing tools, build failure, binary verification failure)

## FILES

- `./target/release/rustowl` - Main binary tested by the script
- `./target/release/rustowlc` - Compiler binary (also built but not directly tested)

## ENVIRONMENT

- `RUSTC_BOOTSTRAP` - Set to 1 for stable compiler with nightly features
- `CARGO_TARGET_DIR` - Cargo build directory (default: target/)
- `NO_COLOR` - Disable colored output

## NOTES

This script is designed for development environments and focuses on testing the current code state without git-based comparisons. It prioritizes reproducibility and comprehensive metrics over raw speed.

For accurate performance measurements, consider running multiple iterations and using appropriate cache management options based on your testing goals.

## SEE ALSO

- `cargo(1)`
- `rustup(1)`
- `hyperfine(1)`
- `valgrind(1)`

RustOwl documentation: docs/CONTRIBUTING.md
