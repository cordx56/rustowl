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
RUST_VERSION="1.87.0"
DUMMY_PACKAGE_PATH="./perf-tests/dummy-package"
BENCHMARK_NAME="rustowl_bench"

# Options
OPEN_REPORT=false
SAVE_BASELINE=""
LOAD_BASELINE=""
COMPARE_MODE=false
CLEAN_BUILD=false
SHOW_OUTPUT=true
REGRESSION_THRESHOLD="2%"

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Performance Benchmarking Script for RustOwl"
    echo "Runs Criterion benchmarks with comparison and regression detection capabilities"
    echo ""
    echo "Options:"
    echo "  -h, --help           Show this help message"
    echo "  -o, --open           Open HTML report in browser after benchmarking"
    echo "  -s, --save NAME      Save benchmark results as baseline with given name"
    echo "  -l, --load NAME      Load and compare against baseline with given name"
    echo "  -c, --compare        Compare current results with 'main' baseline"
    echo "  -C, --clean          Clean build before benchmarking"
    echo "  -q, --quiet          Suppress benchmark output (for CI)"
    echo "  --threshold PCT      Set regression warning threshold (default: 2%)"
    echo "  --list-baselines     List all available baselines"
    echo ""
    echo "Performance Testing:"
    echo "  This script runs the same benchmarks as the CI performance workflow"
    echo "  It tests RustOwl's analysis performance on the dummy package and warns"
    echo "  about regressions above the threshold (default: 2%)"
    echo ""
    echo "Examples:"
    echo "  $0                           # Run benchmarks and show results"
    echo "  $0 --open                    # Run benchmarks and open HTML report"
    echo "  $0 --save my-baseline        # Save results as 'my-baseline'"
    echo "  $0 --load my-baseline        # Compare against 'my-baseline'"
    echo "  $0 --compare                 # Compare against 'main' baseline"
    echo "  $0 --clean --save main       # Clean build, then save as 'main' baseline"
    echo "  $0 --quiet --threshold 5%    # Run quietly with 5% regression threshold"
    echo ""
    echo "Prerequisites:"
    echo "  - Rust toolchain $RUST_VERSION must be installed"
    echo "  - cargo-criterion (optional, for additional features)"
    echo "  - gnuplot (optional, for detailed plots)"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        -o|--open)
            OPEN_REPORT=true
            shift
            ;;
        -s|--save)
            SAVE_BASELINE="$2"
            shift 2
            ;;
        -l|--load)
            LOAD_BASELINE="$2"
            shift 2
            ;;
        -c|--compare)
            COMPARE_MODE=true
            LOAD_BASELINE="main"
            shift
            ;;
        -C|--clean)
            CLEAN_BUILD=true
            shift
            ;;
        -q|--quiet)
            SHOW_OUTPUT=false
            shift
            ;;
        --threshold)
            REGRESSION_THRESHOLD="$2"
            shift 2
            ;;
        --list-baselines)
            if [ -d "target/criterion" ]; then
                echo "Available baselines:"
                find target/criterion -name "base" -type d | sed 's|target/criterion/||; s|/base||' | sort
            else
                echo "No benchmarks have been run yet."
            fi
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

print_header() {
    echo -e "${BLUE}${BOLD}=====================================${NC}"
    echo -e "${BLUE}${BOLD}    RustOwl Performance Benchmark${NC}"
    echo -e "${BLUE}${BOLD}=====================================${NC}"
    echo ""
}

check_prerequisites() {
    echo -e "${YELLOW}Checking prerequisites...${NC}"
    
    # Check Rust toolchain
    if ! rustup run "$RUST_VERSION" rustc --version >/dev/null 2>&1; then
        echo -e "${RED}Error: Rust $RUST_VERSION is not installed${NC}"
        echo "Please install it with: rustup install $RUST_VERSION"
        exit 1
    fi
    
    # Check dummy package
    if [ ! -d "$DUMMY_PACKAGE_PATH" ]; then
        echo -e "${RED}Error: Dummy package not found at $DUMMY_PACKAGE_PATH${NC}"
        exit 1
    fi
    
    if [ ! -f "$DUMMY_PACKAGE_PATH/Cargo.toml" ]; then
        echo -e "${RED}Error: Cargo.toml not found in dummy package${NC}"
        exit 1
    fi
    
    # Check for bc (needed for regression analysis)
    if ! command -v bc >/dev/null 2>&1; then
        echo -e "${YELLOW}! bc (basic calculator) not found - regression analysis will be limited${NC}"
        echo "Install with: sudo apt-get install bc (or equivalent for your system)"
    fi
    
    # Check optional tools
    if command -v gnuplot >/dev/null 2>&1; then
        echo -e "${GREEN}✓ gnuplot found (detailed plots available)${NC}"
    else
        echo -e "${YELLOW}! gnuplot not found (install for detailed plots)${NC}"
    fi
    
    if command -v cargo-criterion >/dev/null 2>&1; then
        echo -e "${GREEN}✓ cargo-criterion found${NC}"
    else
        echo -e "${YELLOW}! cargo-criterion not found (install with: cargo install cargo-criterion)${NC}"
    fi
    
    echo ""
}

clean_build() {
    if [ "$CLEAN_BUILD" = true ]; then
        echo -e "${YELLOW}Cleaning previous build...${NC}"
        cargo clean
        echo ""
    fi
}

build_rustowl() {
    echo -e "${YELLOW}Building RustOwl in release mode...${NC}"
    RUSTC_BOOTSTRAP=1 rustup run "$RUST_VERSION" cargo build --release
    echo ""
}

run_benchmarks() {
    echo -e "${YELLOW}Running Criterion benchmarks...${NC}"
    
    local bench_args=("--bench" "$BENCHMARK_NAME")
    
    if [ -n "$SAVE_BASELINE" ]; then
        bench_args+=("--" "--save-baseline" "$SAVE_BASELINE")
        echo -e "${BLUE}Saving results as baseline: $SAVE_BASELINE${NC}"
    elif [ -n "$LOAD_BASELINE" ]; then
        bench_args+=("--" "--load-baseline" "$LOAD_BASELINE")
        echo -e "${BLUE}Comparing against baseline: $LOAD_BASELINE${NC}"
    fi
    
    # Capture output for regression analysis
    local output_file="/tmp/criterion_output.txt"
    
    if [ "$SHOW_OUTPUT" = true ]; then
        cargo "${bench_args[@]}" | tee "$output_file"
    else
        cargo "${bench_args[@]}" > "$output_file" 2>&1
        echo -e "${GREEN}✓ Benchmarks completed${NC}"
    fi
    
    # Analyze for regressions if comparing
    if [ -n "$LOAD_BASELINE" ]; then
        analyze_regressions "$output_file"
    fi
    
    echo ""
}

# Analyze benchmark output for regressions
analyze_regressions() {
    local output_file="$1"
    
    echo -e "${BLUE}Analyzing performance changes...${NC}"
    
    if ! command -v bc >/dev/null 2>&1; then
        echo -e "${YELLOW}bc not available - showing all detected changes${NC}"
        if grep -E "change:.*\+[0-9]+\.[0-9]+%" "$output_file"; then
            echo -e "${YELLOW}⚠ Performance regressions may be present - install bc for threshold analysis${NC}"
        else
            echo -e "${GREEN}✓ No obvious regressions detected${NC}"
        fi
        return
    fi
    
    # Look for "change:" patterns in Criterion output
    local regressions_found=false
    local threshold_num=$(echo "$REGRESSION_THRESHOLD" | sed 's/%//')
    
    while IFS= read -r line; do
        if echo "$line" | grep -E "change:.*\+[0-9]+\.[0-9]+%" >/dev/null; then
            # Extract the percentage change
            local change=$(echo "$line" | sed -E 's/.*change:.*\+([0-9]+\.[0-9]+)%.*/\1/')
            
            # Compare with threshold (basic floating point comparison)
            if (( $(echo "$change > $threshold_num" | bc -l) )); then
                echo -e "${YELLOW}⚠ Performance regression detected: +${change}% (threshold: $REGRESSION_THRESHOLD)${NC}"
                echo "  $line"
                regressions_found=true
            fi
        fi
    done < "$output_file"
    
    if [ "$regressions_found" = false ]; then
        echo -e "${GREEN}✓ No significant regressions detected (threshold: $REGRESSION_THRESHOLD)${NC}"
    fi
}

open_report() {
    if [ "$OPEN_REPORT" = true ]; then
        local report_path="target/criterion/reports/index.html"
        if [ -f "$report_path" ]; then
            echo -e "${GREEN}Opening HTML report...${NC}"
            case "$(uname)" in
                Darwin)
                    open "$report_path"
                    ;;
                Linux)
                    if command -v xdg-open >/dev/null 2>&1; then
                        xdg-open "$report_path"
                    else
                        echo -e "${YELLOW}Please open $report_path in your browser${NC}"
                    fi
                    ;;
                *)
                    echo -e "${YELLOW}Please open $report_path in your browser${NC}"
                    ;;
            esac
        else
            echo -e "${YELLOW}HTML report not found at $report_path${NC}"
        fi
    fi
}

show_results_location() {
    echo -e "${GREEN}${BOLD}Benchmark completed!${NC}"
    echo ""
    echo -e "${BLUE}Results locations:${NC}"
    echo "  • HTML report: target/criterion/reports/index.html"
    echo "  • Detailed data: target/criterion/"
    echo ""
    
    if [ -n "$SAVE_BASELINE" ]; then
        echo -e "${GREEN}Baseline '$SAVE_BASELINE' saved successfully${NC}"
        echo "Use --load $SAVE_BASELINE to compare future runs against this baseline"
        echo ""
    fi
    
    if [ -n "$LOAD_BASELINE" ]; then
        echo -e "${BLUE}Comparison completed against baseline '$LOAD_BASELINE'${NC}"
        echo "Check the output above for performance differences"
        echo ""
        echo -e "${YELLOW}Tip: Set a different regression threshold with --threshold <percentage>${NC}"
        echo "Current threshold: $REGRESSION_THRESHOLD"
        echo ""
    fi
    
    echo -e "${BLUE}Integration with CI:${NC}"
    echo "This script runs the same benchmarks as the bench-performance.yml workflow"
    echo "Consider saving a 'main' baseline to track performance changes over time"
}

# Main execution
main() {
    print_header
    check_prerequisites
    clean_build
    build_rustowl
    run_benchmarks
    show_results_location
    open_report
}

# Run main function
main "$@"
