#!/bin/bash
# Local performance benchmarking script for RustOwl
# This script provides an easy way to run Criterion benchmarks locally
# Matches the bench-performance.yml CI workflow

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_NAME="rustowl_bench_simple"

# Look for existing test packages in the repo
TEST_PACKAGES=(
    "./tests/fixtures"
    "./benches/fixtures" 
    "./test-data"
    "./examples"
    "./perf-tests"
)

# Options
OPEN_REPORT=false
SAVE_BASELINE=""
LOAD_BASELINE=""
COMPARE_MODE=false
CLEAN_BUILD=false
SHOW_OUTPUT=true
REGRESSION_THRESHOLD="5%"
TEST_PACKAGE_PATH=""

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Performance Benchmarking Script for RustOwl"
    echo "Runs Criterion benchmarks with comparison and regression detection capabilities"
    echo ""
    echo "Options:"
    echo "  -h, --help           Show this help message"
    echo "  --save <name>        Save benchmark results as baseline with given name"
    echo "  --load <name>        Load baseline and compare current results against it"
    echo "  --threshold <percent> Set regression threshold (default: 5%)"
    echo "  --test-package <path> Use specific test package (auto-detected if not specified)"
    echo "  --open               Open HTML report in browser after benchmarking"
    echo "  --clean              Clean build artifacts before benchmarking"
    echo "  --quiet              Minimal output (for CI/automated use)"
    echo ""
    echo "Examples:"
    echo "  $0                           # Run benchmarks with default settings"
    echo "  $0 --save main               # Save results as 'main' baseline"
    echo "  $0 --load main --threshold 3% # Compare against 'main' with 3% threshold"
    echo "  $0 --clean --open            # Clean build, run benchmarks, open report"
    echo "  $0 --save current --quiet    # Save baseline quietly (for CI)"
    echo ""
    echo "Baseline Management:"
    echo "  Baselines are stored in: baselines/performance/<name>/"
    echo "  HTML reports are in: target/criterion/report/"
    echo ""
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        --save)
            if [[ -z "$2" ]]; then
                echo -e "${RED}Error: --save requires a baseline name${NC}"
                echo "Example: $0 --save main"
                exit 1
            fi
            SAVE_BASELINE="$2"
            shift 2
            ;;
        --load)
            if [[ -z "$2" ]]; then
                echo -e "${RED}Error: --load requires a baseline name${NC}"
                echo "Example: $0 --load main"
                exit 1
            fi
            LOAD_BASELINE="$2"
            COMPARE_MODE=true
            shift 2
            ;;
        --threshold)
            if [[ -z "$2" ]]; then
                echo -e "${RED}Error: --threshold requires a percentage${NC}"
                echo "Example: $0 --threshold 3%"
                exit 1
            fi
            REGRESSION_THRESHOLD="$2"
            shift 2
            ;;
        --test-package)
            if [[ -z "$2" ]]; then
                echo -e "${RED}Error: --test-package requires a path${NC}"
                echo "Example: $0 --test-package ./examples/sample"
                exit 1
            fi
            TEST_PACKAGE_PATH="$2"
            shift 2
            ;;
        --open)
            OPEN_REPORT=true
            shift
            ;;
        --clean)
            CLEAN_BUILD=true
            shift
            ;;
        --quiet)
            SHOW_OUTPUT=false
            shift
            ;;
        baseline)
            # Legacy support for CI workflow
            SAVE_BASELINE="main"
            SHOW_OUTPUT=false
            shift
            ;;
        compare)
            # Legacy support for CI workflow
            COMPARE_MODE=true
            LOAD_BASELINE="main"
            shift
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

print_header() {
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${BLUE}${BOLD}=====================================${NC}"
        echo -e "${BLUE}${BOLD}  RustOwl Performance Benchmarks${NC}"
        echo -e "${BLUE}${BOLD}=====================================${NC}"
        echo ""
        
        if [[ -n "$SAVE_BASELINE" ]]; then
            echo -e "${GREEN}Mode: Save baseline as '$SAVE_BASELINE'${NC}"
        elif [[ "$COMPARE_MODE" == "true" ]]; then
            echo -e "${GREEN}Mode: Compare against '$LOAD_BASELINE' baseline${NC}"
            echo -e "${GREEN}Regression threshold: $REGRESSION_THRESHOLD${NC}"
        else
            echo -e "${GREEN}Mode: Standard benchmark run${NC}"
        fi
        echo ""
    fi
}

find_test_package() {
    if [[ -n "$TEST_PACKAGE_PATH" ]]; then
        if [[ -d "$TEST_PACKAGE_PATH" ]]; then
            if [[ "$SHOW_OUTPUT" == "true" ]]; then
                echo -e "${GREEN}✓ Using specified test package: $TEST_PACKAGE_PATH${NC}"
            fi
            return 0
        else
            echo -e "${RED}Error: Specified test package not found: $TEST_PACKAGE_PATH${NC}"
            exit 1
        fi
    fi
    
    # Auto-detect existing test packages
    for test_dir in "${TEST_PACKAGES[@]}"; do
        if [[ -d "$test_dir" ]]; then
            # Check if it contains Rust code
            if find "$test_dir" -name "*.rs" | head -1 >/dev/null 2>&1; then
                TEST_PACKAGE_PATH="$test_dir"
                if [[ "$SHOW_OUTPUT" == "true" ]]; then
                    echo -e "${GREEN}✓ Found test package: $TEST_PACKAGE_PATH${NC}"
                fi
                return 0
            fi
            # Check if it contains Cargo.toml files (subdirectories with packages)
            if find "$test_dir" -name "Cargo.toml" | head -1 >/dev/null 2>&1; then
                TEST_PACKAGE_PATH=$(find "$test_dir" -name "Cargo.toml" | head -1 | xargs dirname)
                if [[ "$SHOW_OUTPUT" == "true" ]]; then
                    echo -e "${GREEN}✓ Found test package: $TEST_PACKAGE_PATH${NC}"
                fi
                return 0
            fi
        fi
    done
    
    # Look for existing benchmark files
    if [[ -d "./benches" ]]; then
        TEST_PACKAGE_PATH="./benches"
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${GREEN}✓ Using benchmark directory: $TEST_PACKAGE_PATH${NC}"
        fi
        return 0
    fi
    
    # Use the current project as test package
    if [[ -f "./Cargo.toml" ]]; then
        TEST_PACKAGE_PATH="."
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${GREEN}✓ Using current project as test package${NC}"
        fi
        return 0
    fi
    
    echo -e "${RED}Error: No suitable test package found in the repository${NC}"
    echo -e "${YELLOW}Searched in: ${TEST_PACKAGES[*]}${NC}"
    echo -e "${YELLOW}Use --test-package <path> to specify a custom location${NC}"
    exit 1
}

check_prerequisites() {
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${YELLOW}Checking prerequisites...${NC}"
    fi
    
    # Check Rust installation (any version is fine - we trust rust-toolchain.toml)
    if ! command -v rustc >/dev/null 2>&1; then
        echo -e "${RED}Error: Rust is not installed${NC}"
        echo -e "${YELLOW}Please install Rust: https://rustup.rs/${NC}"
        exit 1
    fi
    
    # Show current Rust version
    local rust_version=$(rustc --version)
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${GREEN}✓ Rust: $rust_version${NC}"
        echo -e "${GREEN}✓ Cargo: $(cargo --version)${NC}"
        echo -e "${GREEN}✓ Host: $(rustc -vV | grep host | cut -d' ' -f2)${NC}"
    fi
    
    # Check if cargo-criterion is available
    if command -v cargo-criterion >/dev/null 2>&1; then
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${GREEN}✓ cargo-criterion is available${NC}"
        fi
    else
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${YELLOW}! cargo-criterion not found, using cargo bench${NC}"
        fi
    fi
    
    # Find and validate test package
    find_test_package
    
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo ""
    fi
}

clean_build() {
    if [[ "$CLEAN_BUILD" == "true" ]]; then
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${YELLOW}Cleaning build artifacts...${NC}"
        fi
        cargo clean
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${GREEN}✓ Build artifacts cleaned${NC}"
            echo ""
        fi
    fi
}

build_rustowl() {
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${YELLOW}Building RustOwl in release mode...${NC}"
    fi
    
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        RUSTC_BOOTSTRAP=1 cargo build --release
    else
        RUSTC_BOOTSTRAP=1 cargo build --release --quiet
    fi
    
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${GREEN}✓ Build completed${NC}"
        echo ""
    fi
}

run_benchmarks() {
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${YELLOW}Running performance benchmarks...${NC}"
    fi
    
    # Check if we have any benchmark files
    if [[ -d "./benches" ]] && find "./benches" -name "*.rs" | head -1 >/dev/null 2>&1; then
        # Prepare benchmark command
        local bench_cmd="cargo bench"
        local bench_args=""
        
        # Use cargo-criterion if available and not doing baseline operations
        if command -v cargo-criterion >/dev/null 2>&1 && [[ -z "$SAVE_BASELINE" && "$COMPARE_MODE" != "true" ]]; then
            bench_cmd="cargo criterion"
        fi
        
        # Add baseline arguments if saving
        if [[ -n "$SAVE_BASELINE" ]]; then
            bench_args="$bench_args --bench rustowl_bench_simple -- --save-baseline $SAVE_BASELINE"
        fi
        
        # Add baseline arguments if comparing
        if [[ "$COMPARE_MODE" == "true" && -n "$LOAD_BASELINE" ]]; then
            bench_args="$bench_args --bench rustowl_bench_simple -- --baseline $LOAD_BASELINE"
        fi
        
        # If no baseline operations, run all benchmarks
        if [[ -z "$SAVE_BASELINE" && "$COMPARE_MODE" != "true" ]]; then
            bench_args="$bench_args --bench rustowl_bench_simple"
        fi
        
        # Run the benchmarks
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${BLUE}Running: $bench_cmd $bench_args${NC}"
            $bench_cmd $bench_args
        else
            $bench_cmd $bench_args --quiet 2>/dev/null || $bench_cmd $bench_args >/dev/null 2>&1
        fi
    else
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${YELLOW}! No benchmark files found in ./benches, skipping Criterion benchmarks${NC}"
        fi
    fi
    
    # Run specific RustOwl analysis benchmarks using real test data
    if [[ -f "./target/release/rustowl" || -f "./target/release/rustowl.exe" ]]; then
        local rustowl_binary="./target/release/rustowl"
        if [[ -f "./target/release/rustowl.exe" ]]; then
            rustowl_binary="./target/release/rustowl.exe"
        fi
        
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${YELLOW}Running RustOwl analysis benchmark on: $TEST_PACKAGE_PATH${NC}"
        fi
        
        # Time the analysis of the test package
        local start_time=$(date +%s.%N 2>/dev/null || date +%s)
        
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            timeout 120 "$rustowl_binary" check "$TEST_PACKAGE_PATH" 2>/dev/null || true
        else
            timeout 120 "$rustowl_binary" check "$TEST_PACKAGE_PATH" >/dev/null 2>&1 || true
        fi
        
        local end_time=$(date +%s.%N 2>/dev/null || date +%s)
        
        # Calculate duration (handle both nanosecond and second precision)
        local duration
        if command -v bc >/dev/null 2>&1 && [[ "$start_time" == *.* ]]; then
            duration=$(echo "$end_time - $start_time" | bc -l 2>/dev/null || echo "N/A")
        else
            duration=$((end_time - start_time))
        fi
        
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${GREEN}✓ Analysis completed in ${duration}s${NC}"
        fi
        
        # Save timing info for comparison
        if [[ -n "$SAVE_BASELINE" ]]; then
            mkdir -p "baselines/performance/$SAVE_BASELINE"
            echo "$duration" > "baselines/performance/$SAVE_BASELINE/analysis_time.txt"
            echo "$TEST_PACKAGE_PATH" > "baselines/performance/$SAVE_BASELINE/test_package.txt"
        fi
        
        # Compare timing if in compare mode
        if [[ "$COMPARE_MODE" == "true" && -f "baselines/performance/$LOAD_BASELINE/analysis_time.txt" ]]; then
            local baseline_time=$(cat "baselines/performance/$LOAD_BASELINE/analysis_time.txt")
            compare_analysis_times "$baseline_time" "$duration"
        fi
    else
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${YELLOW}! RustOwl binary not found, skipping analysis benchmark${NC}"
        fi
    fi
    
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${GREEN}✓ Benchmarks completed${NC}"
        echo ""
    fi
}

compare_analysis_times() {
    local baseline_time="$1"
    local current_time="$2"
    
    if [[ "$baseline_time" == "N/A" || "$current_time" == "N/A" ]]; then
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${YELLOW}! Could not compare analysis times (timing unavailable)${NC}"
        fi
        return 0
    fi
    
    # Calculate percentage change (handle integer vs float)
    local change="0"
    local abs_change="0"
    
    if command -v bc >/dev/null 2>&1; then
        change=$(echo "scale=2; (($current_time - $baseline_time) / $baseline_time) * 100" | bc -l 2>/dev/null || echo "0")
        abs_change=$(echo "$change" | sed 's/-//')
    else
        # Simple integer math fallback
        if [[ "$current_time" -gt "$baseline_time" ]]; then
            change="positive"
            abs_change="significant"
        elif [[ "$current_time" -lt "$baseline_time" ]]; then
            change="negative"
            abs_change="significant"
        else
            change="0"
            abs_change="0"
        fi
    fi
    
    # Extract threshold number
    local threshold_num=$(echo "$REGRESSION_THRESHOLD" | sed 's/%//')
    
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${BLUE}Analysis Time Comparison:${NC}"
        echo -e "  Baseline: ${baseline_time}s"
        echo -e "  Current:  ${current_time}s"
        echo -e "  Change:   ${change}%"
        
        # Show what test package was used for baseline
        if [[ -f "baselines/performance/$LOAD_BASELINE/test_package.txt" ]]; then
            local baseline_package=$(cat "baselines/performance/$LOAD_BASELINE/test_package.txt")
            echo -e "  Test Package: ${baseline_package}"
        fi
    fi
    
    # Check for regression
    if (( $(echo "$abs_change > $threshold_num" | bc -l 2>/dev/null || echo "0") )); then
        if (( $(echo "$change > 0" | bc -l 2>/dev/null || echo "0") )); then
            if [[ "$SHOW_OUTPUT" == "true" ]]; then
                echo -e "${RED}⚠ Performance regression detected! (+${abs_change}% > ${REGRESSION_THRESHOLD})${NC}"
            fi
            return 1
        else
            if [[ "$SHOW_OUTPUT" == "true" ]]; then
                echo -e "${GREEN}✓ Performance improvement detected! (-${abs_change}%)${NC}"
            fi
        fi
    else
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${GREEN}✓ Performance within acceptable range (±${abs_change}% ≤ ${REGRESSION_THRESHOLD})${NC}"
        fi
    fi
    
    return 0
}

# Analyze benchmark output for regressions
analyze_regressions() {
    if [[ "$COMPARE_MODE" != "true" ]]; then
        return 0
    fi
    
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${YELLOW}Analyzing benchmark results for regressions...${NC}"
    fi
    
    # Look for Criterion output files
    local criterion_dir="target/criterion"
    local regression_found=false
    
    if [[ -d "$criterion_dir" ]]; then
        # Check for regression indicators in Criterion reports (simplified)
        if find "$criterion_dir" -name "*.html" -print0 2>/dev/null | xargs -0 grep -l "regressed\|slower" 2>/dev/null | head -1 >/dev/null; then
            regression_found=true
        fi
        
        # Create a summary file for CI
        if [[ -f "$criterion_dir/report/index.html" ]]; then
            echo "Criterion benchmark report generated: target/criterion/report/index.html" > benchmark-summary.txt
            echo "Test package analyzed: $TEST_PACKAGE_PATH" >> benchmark-summary.txt
            
            # Try to extract some basic statistics
            if command -v grep >/dev/null 2>&1; then
                echo "" >> benchmark-summary.txt
                echo "Quick Summary:" >> benchmark-summary.txt
                find "$criterion_dir" -name "*.json" -exec grep -h "\"mean\"" {} \; | head -5 >> benchmark-summary.txt 2>/dev/null || true
            fi
        fi
    fi
    
    if [[ "$regression_found" == "true" ]]; then
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${RED}⚠ Performance regressions detected in detailed analysis${NC}"
            echo -e "${YELLOW}Check the HTML report for details: target/criterion/report/index.html${NC}"
        fi
        return 1
    else
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${GREEN}✓ No significant regressions detected${NC}"
        fi
        return 0
    fi
}

open_report() {
    if [[ "$OPEN_REPORT" == "true" && -f "target/criterion/report/index.html" ]]; then
        if [[ "$SHOW_OUTPUT" == "true" ]]; then
            echo -e "${YELLOW}Opening benchmark report...${NC}"
        fi
        
        # Try to open the report in the default browser
        if command -v xdg-open >/dev/null 2>&1; then
            xdg-open "target/criterion/report/index.html" 2>/dev/null &
        elif command -v open >/dev/null 2>&1; then
            open "target/criterion/report/index.html" 2>/dev/null &
        elif command -v start >/dev/null 2>&1; then
            start "target/criterion/report/index.html" 2>/dev/null &
        else
            if [[ "$SHOW_OUTPUT" == "true" ]]; then
                echo -e "${YELLOW}Could not auto-open report. Please open: target/criterion/report/index.html${NC}"
            fi
        fi
    fi
}

show_results_location() {
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        echo -e "${BLUE}${BOLD}Results Location:${NC}"
        
        if [[ -f "target/criterion/report/index.html" ]]; then
            echo -e "${GREEN}✓ HTML Report: target/criterion/report/index.html${NC}"
        fi
        
        if [[ -n "$SAVE_BASELINE" && -d "baselines/performance/$SAVE_BASELINE" ]]; then
            echo -e "${GREEN}✓ Saved baseline: baselines/performance/$SAVE_BASELINE/${NC}"
        fi
        
        if [[ -f "benchmark-summary.txt" ]]; then
            echo -e "${GREEN}✓ Summary: benchmark-summary.txt${NC}"
        fi
        
        echo -e "${BLUE}✓ Test package used: $TEST_PACKAGE_PATH${NC}"
        
        echo ""
        echo -e "${YELLOW}Tips:${NC}"
        echo -e "  • Use --open to automatically open the HTML report"
        echo -e "  • Use --save <name> to create a baseline for future comparisons"
        echo -e "  • Use --load <name> to compare against a saved baseline"
        echo -e "  • Use --test-package <path> to benchmark specific test data"
        echo ""
    fi
}

# Main execution
main() {
    print_header
    check_prerequisites
    clean_build
    build_rustowl
    run_benchmarks
    
    # Check for regressions and set exit code
    local exit_code=0
    if ! analyze_regressions; then
        exit_code=1
    fi
    
    open_report
    show_results_location
    
    if [[ "$SHOW_OUTPUT" == "true" ]]; then
        if [[ $exit_code -eq 0 ]]; then
            echo -e "${GREEN}${BOLD}✓ Benchmark completed successfully!${NC}"
        else
            echo -e "${RED}${BOLD}⚠ Benchmark completed with performance regressions detected${NC}"
        fi
    fi
    
    exit $exit_code
}

# Run main function
main "$@"
