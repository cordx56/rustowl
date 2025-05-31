#!/bin/bash
# RustOwl Performance Testing Script
# Tests current code state with configurable metrics and output formats

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Global configuration
OS=$(uname)
RUNS=3
COLD_CACHE=1
OUTPUT_FORMAT="stdio"  # stdio, markdown
OUTPUT_FILE=""
COMPARE_FILE=""
FORCE_BUILD=0
MODE="test"

# Metric flags (default: size and time)
METRIC_SIZE=1
METRIC_TIME=1
METRIC_SYMBOLS=0
METRIC_STRIP=0
METRIC_MEMORY=0
METRIC_PAGE_FAULTS=0
METRIC_CONTEXT_SWITCHES=0
METRIC_STATIC=0
METRIC_PROFILE_MEMORY=0
METRIC_PROFILE_CPU=0

# Available tools detection
HAS_HYPERFINE=0
HAS_VALGRIND=0
HAS_NM=0
HAS_PERF=0
HAS_INSTRUMENTS=0

# Build configuration
RUST_VERSION="1.86.0"
BUILD_CMD="RUSTC_BOOTSTRAP=1 rustup run $RUST_VERSION cargo build --release"
BINARY_PATH="./target/release/rustowl"

# Test results storage for aggregation
declare -a SIZE_RESULTS=()
declare -a TIME_RESULTS=()
declare -a MEMORY_RESULTS=()
declare -a SYMBOLS_RESULTS=()
declare -a PAGE_FAULT_RESULTS=()
declare -a CONTEXT_SWITCH_RESULTS=()

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Modes:"
    echo "  --verify         Check system readiness and tool availability"
    echo "  --prepare        Build the project without running tests"
    echo "  --test           Run performance tests (default if no mode specified)"
    echo ""
    echo "Test Configuration:"
    echo "  --runs N         Number of test runs (default: 3)"
    echo "  --warm           Skip cache clearing between runs"
    echo "  --cold           Clear caches between runs (default)"
    echo "  --force          Force rebuild even if binary exists"
    echo ""
    echo "Metrics (default: size,time):"
    echo "  --full           Enable all available metrics"
    echo "  --size           Binary size analysis"
    echo "  --time           Execution time measurement"
    echo "  --symbols        Symbol count analysis"
    echo "  --strip          Strip analysis (debug symbol overhead)"
    echo "  --memory         Memory usage measurement"
    echo "  --page-faults    Page fault analysis"
    echo "  --context-switches Context switch measurement"
    echo "  --static         Static analysis metrics"
    echo "  --profile-memory Memory profiling (valgrind)"
    echo "  --profile-cpu    CPU profiling recommendations"
    echo ""
    echo "Output:"
    echo "  --stdout         Output to standard output (default)"
    echo "  --markdown FILE  Output results to markdown file"
    echo "  --compare FILE   Compare with existing markdown results"
    echo ""
    echo "Examples:"
    echo "  $0                           # Basic test (size + time, 3 runs, cold)"
    echo "  $0 --verify                  # Check system readiness"
    echo "  $0 --full --runs 5           # Full metrics, 5 runs"
    echo "  $0 --markdown results.md     # Save results to file"
    echo "  $0 --compare old-results.md  # Compare with previous results"
    echo "  $0 --memory --warm --runs 1  # Memory test, warm cache, single run"
    echo ""
}

enable_all_metrics() {
    METRIC_SIZE=1
    METRIC_TIME=1
    METRIC_SYMBOLS=1
    METRIC_STRIP=1
    METRIC_MEMORY=1
    METRIC_PAGE_FAULTS=1
    METRIC_CONTEXT_SWITCHES=1
    METRIC_STATIC=1
    METRIC_PROFILE_MEMORY=1
    METRIC_PROFILE_CPU=1
}

reset_default_metrics() {
    METRIC_SIZE=1
    METRIC_TIME=1
}

# Parse command line arguments
parse_args() {
    local mode_specified=0
    local metrics_specified=0
    local all_args=("$@")
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                usage
                exit 0
                ;;
            --verify)
                MODE="verify"
                mode_specified=1
                shift
                ;;
            --prepare)
                MODE="prepare"
                mode_specified=1
                shift
                ;;
            --test)
                MODE="test"
                mode_specified=1
                shift
                ;;
            --runs)
                RUNS="$2"
                if ! [[ "$RUNS" =~ ^[0-9]+$ ]] || [ "$RUNS" -lt 1 ]; then
                    echo -e "${RED}Error: --runs must be a positive integer${NC}"
                    exit 1
                fi
                shift 2
                ;;
            --warm)
                COLD_CACHE=0
                shift
                ;;
            --cold)
                COLD_CACHE=1
                shift
                ;;
            --force)
                FORCE_BUILD=1
                shift
                ;;
            --full)
                enable_all_metrics
                metrics_specified=1
                shift
                ;;
            --size)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_SIZE=1
                shift
                ;;
            --time)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_TIME=1
                shift
                ;;
            --symbols)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_SYMBOLS=1
                shift
                ;;
            --strip)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_STRIP=1
                shift
                ;;
            --memory)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_MEMORY=1
                shift
                ;;
            --page-faults)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_PAGE_FAULTS=1
                shift
                ;;
            --context-switches)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_CONTEXT_SWITCHES=1
                shift
                ;;
            --static)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_STATIC=1
                shift
                ;;
            --profile-memory)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_PROFILE_MEMORY=1
                shift
                ;;
            --profile-cpu)
                if [ $metrics_specified -eq 0 ]; then
                    reset_default_metrics
                    metrics_specified=1
                fi
                METRIC_PROFILE_CPU=1
                shift
                ;;
            --stdout)
                OUTPUT_FORMAT="stdio"
                shift
                ;;
            --markdown)
                OUTPUT_FORMAT="markdown"
                OUTPUT_FILE="$2"
                if [ -z "$OUTPUT_FILE" ]; then
                    echo -e "${RED}Error: --markdown requires a filename${NC}"
                    exit 1
                fi
                shift 2
                ;;
            --compare)
                COMPARE_FILE="$2"
                if [ -z "$COMPARE_FILE" ] || [ ! -f "$COMPARE_FILE" ]; then
                    echo -e "${RED}Error: --compare requires an existing markdown file${NC}"
                    exit 1
                fi
                shift 2
                ;;
            *)
                echo -e "${RED}Unknown option: $1${NC}"
                usage
                exit 1
                ;;
        esac
    done
    
    # Set default mode if none specified
    if [ $mode_specified -eq 0 ]; then
        MODE="test"
    fi
}

# Detect available tools
detect_tools() {
    if command -v hyperfine >/dev/null 2>&1; then
        HAS_HYPERFINE=1
    fi
    if command -v valgrind >/dev/null 2>&1; then
        HAS_VALGRIND=1
    fi
    if command -v nm >/dev/null 2>&1; then
        HAS_NM=1
    fi
    if command -v perf >/dev/null 2>&1; then
        HAS_PERF=1
    fi
    if command -v instruments >/dev/null 2>&1; then
        HAS_INSTRUMENTS=1
    fi
}

# Check required tools
check_required_tools() {
    local missing_tools=()
    
    command -v rustup >/dev/null 2>&1 || missing_tools+=("rustup")
    command -v cargo >/dev/null 2>&1 || missing_tools+=("cargo")
    command -v bc >/dev/null 2>&1 || missing_tools+=("bc")
    
    # Check if specific Rust version is available
    if ! rustup run "$RUST_VERSION" rustc --version >/dev/null 2>&1; then
        missing_tools+=("rust-$RUST_VERSION")
    fi
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        echo -e "${RED}Error: Missing required tools: ${missing_tools[*]}${NC}"
        echo "Install missing tools:"
        for tool in "${missing_tools[@]}"; do
            case $tool in
                rustup) echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" ;;
                cargo) echo "  (included with rustup)" ;;
                bc) echo "  macOS: brew install bc | Linux: apt/yum install bc" ;;
                rust-$RUST_VERSION) echo "  rustup install $RUST_VERSION" ;;
            esac
        done
        exit 1
    fi
}

print_header() {
    if [ "$OUTPUT_FORMAT" = "stdio" ]; then
        echo -e "${BLUE}${BOLD}RustOwl Performance Testing${NC}"
        echo -e "${BLUE}===========================${NC}"
        echo "OS: $OS"
        echo "Rust version: $RUST_VERSION"
        echo "Runs: $RUNS"
        echo "Cache mode: $([ $COLD_CACHE -eq 1 ] && echo "Cold" || echo "Warm")"
        echo "Mode: $MODE"
        echo ""
    fi
}

# Clear system caches
clear_caches() {
    if [ $COLD_CACHE -eq 0 ]; then
        return
    fi
    
    case "$OS" in
        Darwin)
            if command -v purge >/dev/null 2>&1; then
                echo "  Clearing macOS caches..."
                if purge 2>/dev/null; then
                    echo "    ✓ System cache cleared"
                else
                    echo "    ℹ System cache clearing skipped (would require sudo)"
                fi
            fi
            ;;
        Linux)
            echo "  Clearing Linux filesystem cache..."
            if command -v sudo >/dev/null 2>&1; then
                sync
                if echo 3 | sudo tee /proc/sys/vm/drop_caches >/dev/null 2>&1; then
                    echo "    ✓ System cache cleared"
                    sudo ldconfig 2>/dev/null || true
                else
                    echo "    ℹ System cache clearing skipped (would require root)"
                fi
            fi
            ;;
    esac
    
    # Clear RustOwl analysis cache
    if [ -d "./target/owl" ]; then
        echo "  Clearing RustOwl analysis cache..."
        rm -rf ./target/owl 2>/dev/null || true
        echo "    ✓ RustOwl analysis cache cleared"
    fi
    
    # Clear Rust incremental compilation cache
    if [ -d "./target" ]; then
        echo "  Clearing Rust incremental cache..."
        rm -rf ./target/release/incremental 2>/dev/null || true
        echo "    ✓ Rust incremental cache cleared"
    fi
    
    sleep 1
}

verify_system() {
    echo -e "${BLUE}System Verification${NC}"
    echo "=================="
    
    check_required_tools
    detect_tools
    
    echo -e "${GREEN}✓ Required tools available${NC}"
    echo ""
    
    echo "Tool availability:"
    echo "  rustup: ✓"
    echo "  cargo: ✓"
    echo "  bc: ✓"
    echo "  rust $RUST_VERSION: ✓"
    echo "  hyperfine: $([ $HAS_HYPERFINE -eq 1 ] && echo "✓" || echo "✗")"
    echo "  valgrind: $([ $HAS_VALGRIND -eq 1 ] && echo "✓" || echo "✗")"
    echo "  nm: $([ $HAS_NM -eq 1 ] && echo "✓" || echo "✗")"
    echo "  perf: $([ $HAS_PERF -eq 1 ] && echo "✓" || echo "✗")"
    echo "  instruments: $([ $HAS_INSTRUMENTS -eq 1 ] && echo "✓" || echo "✗")"
    echo ""
    
    echo "Enabled metrics:"
    [ $METRIC_SIZE -eq 1 ] && echo "  ✓ Binary size"
    [ $METRIC_TIME -eq 1 ] && echo "  ✓ Execution time"
    [ $METRIC_SYMBOLS -eq 1 ] && echo "  ✓ Symbol analysis"
    [ $METRIC_STRIP -eq 1 ] && echo "  ✓ Strip analysis"
    [ $METRIC_MEMORY -eq 1 ] && echo "  ✓ Memory usage"
    [ $METRIC_PAGE_FAULTS -eq 1 ] && echo "  ✓ Page faults"
    [ $METRIC_CONTEXT_SWITCHES -eq 1 ] && echo "  ✓ Context switches"
    [ $METRIC_STATIC -eq 1 ] && echo "  ✓ Static analysis"
    [ $METRIC_PROFILE_MEMORY -eq 1 ] && echo "  ✓ Memory profiling"
    [ $METRIC_PROFILE_CPU -eq 1 ] && echo "  ✓ CPU profiling"
    echo ""
    
    if [ ! -z "$OUTPUT_FILE" ]; then
        echo "Output file: $OUTPUT_FILE"
    fi
    
    if [ ! -z "$COMPARE_FILE" ]; then
        echo "Comparison file: $COMPARE_FILE"
    fi
}

build_project() {
    echo -e "${BLUE}Building Project${NC}"
    echo "================"
    
    # Check if rebuild is needed
    if [ -f "$BINARY_PATH" ] && [ $FORCE_BUILD -eq 0 ]; then
        echo -e "${YELLOW}Binary exists. Use --force to rebuild.${NC}"
        return 0
    fi
    
    echo "Build command: $BUILD_CMD"
    echo ""
    
    # Clean previous build if forced
    if [ $FORCE_BUILD -eq 1 ]; then
        echo "Cleaning previous build..."
        cargo clean --release 2>/dev/null || true
    fi
    
    echo "Building release binary..."
    if ! eval "$BUILD_CMD"; then
        echo -e "${RED}✗ Build failed${NC}"
        exit 1
    fi
    
    if [ ! -f "$BINARY_PATH" ]; then
        echo -e "${RED}✗ Binary not found after build: $BINARY_PATH${NC}"
        exit 1
    fi
    
    # Verify binary works
    if ! "$BINARY_PATH" --version >/dev/null 2>&1; then
        echo -e "${RED}✗ Binary verification failed${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓ Build completed successfully${NC}"
    echo ""
}

# Metric measurement functions
measure_size() {
    if [ $METRIC_SIZE -eq 0 ]; then return; fi
    
    local size_bytes=$(stat -f%z "$BINARY_PATH" 2>/dev/null || stat -c%s "$BINARY_PATH" 2>/dev/null)
    local size_human=$(ls -lh "$BINARY_PATH" | awk '{print $5}')
    
    SIZE_RESULTS+=("$size_bytes")
    echo "Binary size: $size_human ($size_bytes bytes)"
}

measure_symbols() {
    if [ $METRIC_SYMBOLS -eq 0 ] || [ $HAS_NM -eq 0 ]; then return; fi
    
    local symbols=$(nm "$BINARY_PATH" 2>/dev/null | wc -l | tr -d ' ')
    SYMBOLS_RESULTS+=("$symbols")
    echo "Symbol count: $symbols"
}

measure_strip() {
    if [ $METRIC_STRIP -eq 0 ]; then return; fi
    
    local temp_binary="/tmp/$(basename $BINARY_PATH)_stripped"
    cp "$BINARY_PATH" "$temp_binary"
    strip "$temp_binary" 2>/dev/null
    local original_size=$(stat -f%z "$BINARY_PATH" 2>/dev/null || stat -c%s "$BINARY_PATH" 2>/dev/null)
    local stripped_size=$(stat -f%z "$temp_binary" 2>/dev/null || stat -c%s "$temp_binary" 2>/dev/null)
    local savings=$((original_size - stripped_size))
    local savings_percent=$(echo "scale=1; ($savings * 100.0) / $original_size" | bc -l)
    
    echo "Stripped size: $(ls -lh $temp_binary | awk '{print $5}') (saves $savings_percent%)"
    rm -f "$temp_binary"
}

measure_time_and_resources() {
    if [ $METRIC_TIME -eq 0 ] && [ $METRIC_MEMORY -eq 0 ] && [ $METRIC_PAGE_FAULTS -eq 0 ] && [ $METRIC_CONTEXT_SWITCHES -eq 0 ]; then
        return
    fi
    
    clear_caches
    
    case "$OS" in
        Darwin)
            # Use manual timing for clean output, /usr/bin/time for system metrics
            if [ $METRIC_TIME -eq 1 ]; then
                local start_time=$(date +%s.%N)
                "$BINARY_PATH" check >/dev/null 2>&1 || true
                local end_time=$(date +%s.%N)
                local real_time=$(echo "scale=2; $end_time - $start_time" | bc -l)
                TIME_RESULTS+=("$real_time")
                echo "Execution time: ${real_time}s"
            fi
            
            # Get system metrics if needed
            if [ $METRIC_MEMORY -eq 1 ] || [ $METRIC_PAGE_FAULTS -eq 1 ] || [ $METRIC_CONTEXT_SWITCHES -eq 1 ]; then
                # Create temporary files to separate output
                local time_stderr=$(mktemp)
                local cmd_output=$(mktemp)
                
                # Run with time and separate outputs properly
                /usr/bin/time -l "$BINARY_PATH" check >"$cmd_output" 2>"$time_stderr" || true
                
                local time_output=$(cat "$time_stderr")
                rm -f "$time_stderr" "$cmd_output"
                
                if [ $METRIC_MEMORY -eq 1 ]; then
                    local memory_bytes=$(echo "$time_output" | grep "maximum resident set size" | awk '{print $1}')
                    local memory_mb=$((memory_bytes / 1024 / 1024))
                    MEMORY_RESULTS+=("$memory_mb")
                    echo "Peak memory: ${memory_mb} MB (${memory_bytes} bytes)"
                fi
                
                if [ $METRIC_PAGE_FAULTS -eq 1 ]; then
                    local page_faults=$(echo "$time_output" | grep "page reclaims" | awk '{print $1}')
                    PAGE_FAULT_RESULTS+=("$page_faults")
                    echo "Page faults: $page_faults"
                fi
                
                if [ $METRIC_CONTEXT_SWITCHES -eq 1 ]; then
                    local vol_cs=$(echo "$time_output" | grep "voluntary context switches" | grep -v "involuntary" | awk '{print $1}')
                    local invol_cs=$(echo "$time_output" | grep "involuntary context switches" | awk '{print $1}')
                    CONTEXT_SWITCH_RESULTS+=("$vol_cs")
                    echo "Voluntary context switches: $vol_cs"
                    echo "Involuntary context switches: $invol_cs"
                fi
            fi
            ;;
        Linux)
            if [ $METRIC_TIME -eq 1 ]; then
                local start_time=$(date +%s.%N)
                "$BINARY_PATH" check >/dev/null 2>&1 || true
                local end_time=$(date +%s.%N)
                local real_time=$(echo "scale=2; $end_time - $start_time" | bc -l)
                TIME_RESULTS+=("$real_time")
                echo "Execution time: $real_time"
            fi
            
            if [ $METRIC_MEMORY -eq 1 ] || [ $METRIC_PAGE_FAULTS -eq 1 ] || [ $METRIC_CONTEXT_SWITCHES -eq 1 ]; then
                local time_output=$(/usr/bin/time -v "$BINARY_PATH" check >/dev/null 2>&1 || true)
                
                if [ $METRIC_MEMORY -eq 1 ]; then
                    local memory_kb=$(echo "$time_output" | grep "Maximum resident set size" | awk '{print $NF}')
                    local memory_mb=$((memory_kb / 1024))
                    MEMORY_RESULTS+=("$memory_mb")
                    echo "Peak memory: ${memory_mb} MB (${memory_kb} KB)"
                fi
                
                if [ $METRIC_PAGE_FAULTS -eq 1 ]; then
                    local page_faults=$(echo "$time_output" | grep "Major (requiring I/O) page faults" | awk '{print $NF}')
                    PAGE_FAULT_RESULTS+=("$page_faults")
                    echo "Page faults: $page_faults"
                fi
                
                if [ $METRIC_CONTEXT_SWITCHES -eq 1 ]; then
                    local vol_cs=$(echo "$time_output" | grep "Voluntary context switches" | awk '{print $NF}')
                    local invol_cs=$(echo "$time_output" | grep "Involuntary context switches" | awk '{print $NF}')
                    CONTEXT_SWITCH_RESULTS+=("$vol_cs")
                    echo "Voluntary context switches: $vol_cs"
                    echo "Involuntary context switches: $invol_cs"
                fi
            fi
            ;;
    esac
}

measure_static_analysis() {
    if [ $METRIC_STATIC -eq 0 ]; then return; fi
    
    echo "Static analysis metrics:"
    
    # File size metrics
    local size_bytes=$(stat -f%z "$BINARY_PATH" 2>/dev/null || stat -c%s "$BINARY_PATH" 2>/dev/null)
    echo "  Binary entropy: $(echo "$size_bytes" | md5sum | cut -c1-8)"
    
    # Dependencies (if available)
    if command -v ldd >/dev/null 2>&1; then
        local deps=$(ldd "$BINARY_PATH" 2>/dev/null | wc -l)
        echo "  Dynamic dependencies: $deps"
    elif command -v otool >/dev/null 2>&1; then
        local deps=$(otool -L "$BINARY_PATH" 2>/dev/null | tail -n +2 | wc -l)
        echo "  Dynamic dependencies: $deps"
    fi
}

profile_memory() {
    if [ $METRIC_PROFILE_MEMORY -eq 0 ] || [ $HAS_VALGRIND -eq 0 ]; then return; fi
    
    echo "Memory profiling (valgrind):"
    clear_caches
    
    valgrind --tool=massif --stacks=yes --time-unit=B \
        --massif-out-file=/tmp/massif.out \
        "$BINARY_PATH" check >/dev/null 2>&1
    
    if [ -f "/tmp/massif.out" ]; then
        local peak_mem=$(grep "mem_heap_B=" /tmp/massif.out | sort -t= -k2 -n | tail -1 | cut -d= -f2)
        local peak_mb=$((peak_mem / 1024 / 1024))
        echo "  Peak heap usage: ${peak_mb} MB"
        rm -f /tmp/massif.out
    fi
}

profile_cpu() {
    if [ $METRIC_PROFILE_CPU -eq 0 ]; then return; fi
    
    echo "CPU profiling recommendations:"
    
    case "$OS" in
        Darwin)
            if [ $HAS_INSTRUMENTS -eq 1 ]; then
                echo "  Run: instruments -t 'CPU Profiler' $BINARY_PATH check"
            else
                echo "  Install Xcode command line tools for 'instruments'"
            fi
            ;;
        Linux)
            if [ $HAS_PERF -eq 1 ]; then
                echo "  Run: perf record -g $BINARY_PATH check && perf report"
            else
                echo "  Install 'perf' for CPU profiling"
            fi
            ;;
    esac
}

calculate_averages() {
    if [ ${#TIME_RESULTS[@]} -gt 0 ]; then
        local sum=0
        for time in "${TIME_RESULTS[@]}"; do
            # Convert time to integer (multiply by 100 to handle decimals)
            local time_int=$(echo "$time * 100" | bc -l | cut -d. -f1)
            sum=$((sum + time_int))
        done
        AVG_TIME=$(echo "scale=2; $sum / ${#TIME_RESULTS[@]} / 100" | bc -l)
    fi
    
    if [ ${#SIZE_RESULTS[@]} -gt 0 ]; then
        local sum=0
        for size in "${SIZE_RESULTS[@]}"; do
            sum=$((sum + size))
        done
        AVG_SIZE=$((sum / ${#SIZE_RESULTS[@]}))
    fi
    
    if [ ${#MEMORY_RESULTS[@]} -gt 0 ]; then
        local sum=0
        for mem in "${MEMORY_RESULTS[@]}"; do
            sum=$((sum + mem))
        done
        AVG_MEMORY=$((sum / ${#MEMORY_RESULTS[@]}))
    fi
    
    if [ ${#PAGE_FAULT_RESULTS[@]} -gt 0 ]; then
        local sum=0
        for pf in "${PAGE_FAULT_RESULTS[@]}"; do
            sum=$((sum + pf))
        done
        AVG_PAGE_FAULTS=$((sum / ${#PAGE_FAULT_RESULTS[@]}))
    fi
    
    if [ ${#CONTEXT_SWITCH_RESULTS[@]} -gt 0 ]; then
        local sum=0
        for cs in "${CONTEXT_SWITCH_RESULTS[@]}"; do
            sum=$((sum + cs))
        done
        AVG_CONTEXT_SWITCHES=$((sum / ${#CONTEXT_SWITCH_RESULTS[@]}))
    fi
    
    if [ ${#SYMBOLS_RESULTS[@]} -gt 0 ]; then
        local sum=0
        for sym in "${SYMBOLS_RESULTS[@]}"; do
            sum=$((sum + sym))
        done
        AVG_SYMBOLS=$((sum / ${#SYMBOLS_RESULTS[@]}))
    fi
}

run_single_test() {
    local run_num=$1
    
    if [ "$OUTPUT_FORMAT" = "stdio" ]; then
        echo -e "${BLUE}Run $run_num/$RUNS${NC}"
        echo "--------"
    fi
    
    measure_size
    measure_symbols
    measure_strip
    measure_time_and_resources
    measure_static_analysis
    profile_memory
    profile_cpu
    
    if [ "$OUTPUT_FORMAT" = "stdio" ]; then
        echo ""
    fi
}

run_performance_tests() {
    echo -e "${BLUE}Performance Testing${NC}"
    echo "==================="
    
    if [ ! -f "$BINARY_PATH" ]; then
        echo -e "${RED}✗ Test binary not found. Run --prepare first.${NC}"
        exit 1
    fi
    
    for ((i=1; i<=RUNS; i++)); do
        run_single_test $i
    done
    
    # Calculate averages (always, even for single runs)
    calculate_averages
    
    # Display summary if multiple runs
    if [ $RUNS -gt 1 ]; then
        echo -e "${BLUE}Summary (averaged over $RUNS runs)${NC}"
        echo "=================================="
        [ ! -z "$AVG_SIZE" ] && echo "Average binary size: $(ls -lh $BINARY_PATH | awk '{print $5}') ($AVG_SIZE bytes)"
        [ ! -z "$AVG_TIME" ] && echo "Average execution time: ${AVG_TIME}s"
        [ ! -z "$AVG_MEMORY" ] && echo "Average peak memory: ${AVG_MEMORY} MB"
        [ ! -z "$AVG_SYMBOLS" ] && echo "Average symbol count: $AVG_SYMBOLS"
        echo ""
    fi
    
    echo -e "${GREEN}✓ Performance testing completed${NC}"
}

output_to_markdown() {
    echo "# RustOwl Performance Test Results" > "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "**Date:** $(date)" >> "$OUTPUT_FILE"
    echo "**OS:** $OS" >> "$OUTPUT_FILE"
    echo "**Rust Version:** $RUST_VERSION" >> "$OUTPUT_FILE"
    echo "**Runs:** $RUNS" >> "$OUTPUT_FILE"
    echo "**Cache Mode:** $([ $COLD_CACHE -eq 1 ] && echo "Cold" || echo "Warm")" >> "$OUTPUT_FILE"
    echo "**Build Command:** \`RUSTC_BOOTSTRAP=1 rustup run 1.86.0 cargo build --release\`" >> "$OUTPUT_FILE"
    echo "**Test Command:** \`rustowl check\`" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    
    # Summary table
    echo "## Performance Summary" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "| Metric | Value | Unit |" >> "$OUTPUT_FILE"
    echo "|--------|-------|------|" >> "$OUTPUT_FILE"
    
    if [ ! -z "$AVG_SIZE" ]; then
        local size_human=$(ls -lh "$BINARY_PATH" | awk '{print $5}')
        echo "| Binary Size | $size_human | ($AVG_SIZE bytes) |" >> "$OUTPUT_FILE"
    fi
    
    if [ ! -z "$AVG_TIME" ]; then
        echo "| Average Execution Time | ${AVG_TIME}s | seconds |" >> "$OUTPUT_FILE"
    fi
    
    if [ ! -z "$AVG_MEMORY" ]; then
        echo "| Average Peak Memory | ${AVG_MEMORY} MB | megabytes |" >> "$OUTPUT_FILE"
    fi
    
    if [ ! -z "$AVG_PAGE_FAULTS" ]; then
        echo "| Average Page Faults | $AVG_PAGE_FAULTS | count |" >> "$OUTPUT_FILE"
    fi
    
    if [ ! -z "$AVG_CONTEXT_SWITCHES" ]; then
        echo "| Average Context Switches | $AVG_CONTEXT_SWITCHES | count |" >> "$OUTPUT_FILE"
    fi
    
    if [ ! -z "$AVG_SYMBOLS" ]; then
        echo "| Average Symbol Count | $AVG_SYMBOLS | symbols |" >> "$OUTPUT_FILE"
    fi
    
    if [ ! -z "$STRIPPED_SIZE" ]; then
        local stripped_human=$(echo "$STRIPPED_SIZE" | awk '{printf "%.1fM", $1/1024/1024}')
        local reduction=$((AVG_SIZE - STRIPPED_SIZE))
        local reduction_percent=$(echo "scale=1; ($reduction * 100.0) / $AVG_SIZE" | bc -l)
        echo "| Stripped Binary Size | $stripped_human | ($STRIPPED_SIZE bytes, -$reduction_percent%) |" >> "$OUTPUT_FILE"
    fi
    echo "" >> "$OUTPUT_FILE"
    
    # Individual run data if multiple runs
    if [ $RUNS -gt 1 ]; then
        echo "## Individual Run Results" >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
        
        if [ ${#TIME_RESULTS[@]} -gt 0 ]; then
            echo "### Execution Times" >> "$OUTPUT_FILE"
            echo "" >> "$OUTPUT_FILE"
            echo "| Run | Time (seconds) |" >> "$OUTPUT_FILE"
            echo "|-----|----------------|" >> "$OUTPUT_FILE"
            for i in "${!TIME_RESULTS[@]}"; do
                echo "| $((i+1)) | ${TIME_RESULTS[$i]}s |" >> "$OUTPUT_FILE"
            done
            echo "" >> "$OUTPUT_FILE"
        fi
        
        if [ ${#MEMORY_RESULTS[@]} -gt 0 ]; then
            echo "### Peak Memory Usage" >> "$OUTPUT_FILE"
            echo "" >> "$OUTPUT_FILE"
            echo "| Run | Memory (MB) |" >> "$OUTPUT_FILE"
            echo "|-----|-------------|" >> "$OUTPUT_FILE"
            for i in "${!MEMORY_RESULTS[@]}"; do
                echo "| $((i+1)) | ${MEMORY_RESULTS[$i]} MB |" >> "$OUTPUT_FILE"
            done
            echo "" >> "$OUTPUT_FILE"
        fi
        
        if [ ${#PAGE_FAULT_RESULTS[@]} -gt 0 ]; then
            echo "### Page Faults" >> "$OUTPUT_FILE"
            echo "" >> "$OUTPUT_FILE"
            echo "| Run | Page Faults |" >> "$OUTPUT_FILE"
            echo "|-----|-------------|" >> "$OUTPUT_FILE"
            for i in "${!PAGE_FAULT_RESULTS[@]}"; do
                echo "| $((i+1)) | ${PAGE_FAULT_RESULTS[$i]} |" >> "$OUTPUT_FILE"
            done
            echo "" >> "$OUTPUT_FILE"
        fi
        
        if [ ${#CONTEXT_SWITCH_RESULTS[@]} -gt 0 ]; then
            echo "### Context Switches" >> "$OUTPUT_FILE"
            echo "" >> "$OUTPUT_FILE"
            echo "| Run | Context Switches |" >> "$OUTPUT_FILE"
            echo "|-----|------------------|" >> "$OUTPUT_FILE"
            for i in "${!CONTEXT_SWITCH_RESULTS[@]}"; do
                echo "| $((i+1)) | ${CONTEXT_SWITCH_RESULTS[$i]} |" >> "$OUTPUT_FILE"
            done
            echo "" >> "$OUTPUT_FILE"
        fi
    fi
    
    # System information
    echo "## System Information" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "- **Architecture:** $(uname -m)" >> "$OUTPUT_FILE"
    echo "- **Kernel:** $(uname -r)" >> "$OUTPUT_FILE"
    if command -v sysctl >/dev/null 2>&1; then
        echo "- **CPU:** $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "Unknown")" >> "$OUTPUT_FILE"
        echo "- **CPU Cores:** $(sysctl -n hw.ncpu 2>/dev/null || echo "Unknown")" >> "$OUTPUT_FILE"
        echo "- **Total Memory:** $(echo "scale=1; $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1024 / 1024 / 1024" | bc -l)GB" >> "$OUTPUT_FILE"
    fi
    echo "" >> "$OUTPUT_FILE"
    
    # Build information
    echo "## Build Information" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "- **Target:** $(rustc --version --verbose | grep "host:" | cut -d' ' -f2)" >> "$OUTPUT_FILE"
    echo "- **Profile:** release" >> "$OUTPUT_FILE"
    echo "- **Optimization:** \`opt-level = 3\`" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    
    echo "---" >> "$OUTPUT_FILE"
    echo "*Generated by RustOwl performance testing script v1.0*" >> "$OUTPUT_FILE"
    
    echo -e "${GREEN}✓ Comprehensive results saved to $OUTPUT_FILE${NC}"
}

compare_with_file() {
    echo -e "${BLUE}Comparing with $COMPARE_FILE${NC}"
    echo "================================="
    
    if [ ! -f "$COMPARE_FILE" ]; then
        echo -e "${RED}✗ Comparison file not found: $COMPARE_FILE${NC}"
        return
    fi
    
    # Extract previous results
    local prev_size=$(grep "Binary Size:" "$COMPARE_FILE" 2>/dev/null | sed 's/.*(\([0-9]*\) bytes).*/\1/')
    local prev_memory=$(grep "Average Peak Memory:" "$COMPARE_FILE" 2>/dev/null | sed 's/.*Memory:\** \([0-9]*\) MB.*/\1/')
    local prev_symbols=$(grep "Average Symbol Count:" "$COMPARE_FILE" 2>/dev/null | sed 's/.*Count:\** \([0-9]*\).*/\1/')
    
    echo "Comparison Results:"
    echo "==================="
    
    if [ ! -z "$AVG_SIZE" ] && [ ! -z "$prev_size" ]; then
        local size_diff=$((AVG_SIZE - prev_size))
        local size_percent=$(echo "scale=1; ($size_diff * 100.0) / $prev_size" | bc -l)
        local size_human=$(ls -lh "$BINARY_PATH" | awk '{print $5}')
        
        if [ $size_diff -gt 0 ]; then
            echo -e "Binary Size: $size_human (${RED}+$size_diff bytes, +$size_percent%${NC})"
        elif [ $size_diff -lt 0 ]; then
            echo -e "Binary Size: $size_human (${GREEN}$size_diff bytes, $size_percent%${NC})"
        else
            echo "Binary Size: $size_human (no change)"
        fi
    fi
    
    if [ ! -z "$AVG_MEMORY" ] && [ ! -z "$prev_memory" ]; then
        local mem_diff=$((AVG_MEMORY - prev_memory))
        local mem_percent=$(echo "scale=1; ($mem_diff * 100.0) / $prev_memory" | bc -l)
        
        if [ $mem_diff -gt 0 ]; then
            echo -e "Peak Memory: ${AVG_MEMORY} MB (${RED}+$mem_diff MB, +$mem_percent%${NC})"
        elif [ $mem_diff -lt 0 ]; then
            echo -e "Peak Memory: ${AVG_MEMORY} MB (${GREEN}$mem_diff MB, $mem_percent%${NC})"
        else
            echo "Peak Memory: ${AVG_MEMORY} MB (no change)"
        fi
    fi
    
    if [ ! -z "$AVG_SYMBOLS" ] && [ ! -z "$prev_symbols" ]; then
        local sym_diff=$((AVG_SYMBOLS - prev_symbols))
        local sym_percent=$(echo "scale=1; ($sym_diff * 100.0) / $prev_symbols" | bc -l)
        
        if [ $sym_diff -gt 0 ]; then
            echo -e "Symbol Count: $AVG_SYMBOLS (${RED}+$sym_diff symbols, +$sym_percent%${NC})"
        elif [ $sym_diff -lt 0 ]; then
            echo -e "Symbol Count: $AVG_SYMBOLS (${GREEN}$sym_diff symbols, $sym_percent%${NC})"
        else
            echo "Symbol Count: $AVG_SYMBOLS (no change)"
        fi
    fi
    
    echo ""
}

cleanup() {
    # No longer need to clean up TEST_BINARY since we use BINARY_PATH directly
    true
}

# Main execution
main() {
    parse_args "$@"
    print_header
    check_required_tools
    detect_tools
    
    case "$MODE" in
        verify)
            verify_system
            ;;
        prepare)
            build_project
            ;;
        test)
            build_project
            run_performance_tests
            
            if [ "$OUTPUT_FORMAT" = "markdown" ]; then
                output_to_markdown
            fi
            
            if [ ! -z "$COMPARE_FILE" ]; then
                compare_with_file
            fi
            ;;
        *)
            echo -e "${RED}Unknown mode: $MODE${NC}"
            usage
            exit 1
            ;;
    esac
    
    cleanup
}

# Trap cleanup on exit
trap cleanup EXIT

# Run main function
main "$@"
