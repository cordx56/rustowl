#!/bin/bash
# Comprehensive Performance Test: Current vs Baseline RustOwl
# Supports macOS and Linux with automatic tool detection

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Global variables
OS=$(uname)
MODE="full"  # full, prepare, compare, verify, stability
FORCE_REBUILD=0
COLD_RUN=0
STASH_CREATED=1

# Usage function
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -h, --help       Show this help message"
    echo "  -p, --prepare    Prepare test environment (build both versions without running tests)"
    echo "  -c, --compare    Compare pre-built binaries (requires --prepare first)"
    echo "  -s, --stability  Run stability test with multiple iterations to detect suspicious results"
    echo "  -v, --verify     Verify binaries and environment without running tests"
    echo "  -f, --force      Force rebuild even if binaries exist"
    echo "  --cold           Clear caches before testing for truly cold runs"
    echo ""
    echo "Default: Full test (prepare + compare)"
    echo ""
    echo "Examples:"
    echo "  $0                # Full automated test"
    echo "  $0 --prepare      # Just build both versions"
    echo "  $0 --verify       # Check what would be tested"
    echo "  $0 --compare      # Compare existing binaries"
    echo "  $0 --stability    # Multi-iteration test to detect measurement issues"
    echo "  $0 --cold         # Clear caches before testing"
    echo "  $0 --stability --cold  # Cold stability test"
    echo ""
    echo "Recommended workflow for suspicious results:"
    echo "  1. $0 --verify    # Check environment and git status"
    echo "  2. $0 --prepare   # Build both versions separately"
    echo "  3. $0 --stability # Run multiple iterations to check consistency"
    echo ""
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        -p|--prepare)
            MODE="prepare"
            shift
            ;;
        -c|--compare)
            MODE="compare"
            shift
            ;;
        -s|--stability)
            MODE="stability"
            shift
            ;;
        -v|--verify)
            MODE="verify"
            shift
            ;;
        -f|--force)
            FORCE_REBUILD=1
            shift
            ;;
        --cold)
            COLD_RUN=1
            shift
            ;;
        *)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

echo -e "${BLUE}Comprehensive Performance Test: Current vs Baseline RustOwl${NC}"
echo -e "${BLUE}================================================================${NC}"
echo "Detected OS: $OS"
echo "Mode: $MODE"
if [ $COLD_RUN -eq 1 ]; then
    echo -e "Cache clearing: ${YELLOW}ENABLED${NC} (cold runs)"
else
    echo -e "Cache clearing: ${BLUE}DISABLED${NC} (warm runs)"
fi
echo ""

# Check required tools and provide installation instructions
check_tools() {
    local missing_tools=()
    
    # Check basic tools
    if ! command -v git >/dev/null 2>&1; then
        missing_tools+=("git")
    fi
    
    if ! command -v cargo >/dev/null 2>&1; then
        missing_tools+=("cargo (Rust toolchain)")
    fi
    
    # OS-specific tools
    case "$OS" in
        Darwin)
            # macOS specific checks
            if ! command -v bc >/dev/null 2>&1; then
                missing_tools+=("bc")
            fi
            ;;
        Linux)
            # Linux specific checks
            if ! command -v bc >/dev/null 2>&1; then
                missing_tools+=("bc")
            fi
            ;;
    esac
    
    # Optional but recommended tools
    echo "Checking for optional performance tools:"
    if command -v hyperfine >/dev/null 2>&1; then
        echo -e "  ${GREEN}✓ hyperfine found${NC} (enhanced benchmarking)"
        HAS_HYPERFINE=1
    else
        echo -e "  ${YELLOW}⚠ hyperfine not found${NC} (install for better benchmarks)"
        HAS_HYPERFINE=0
    fi
    
    if command -v valgrind >/dev/null 2>&1 && [ "$OS" = "Linux" ]; then
        echo -e "  ${GREEN}✓ valgrind found${NC} (memory profiling)"
        HAS_VALGRIND=1
    else
        echo -e "  ${YELLOW}⚠ valgrind not found${NC} (Linux only, install for memory profiling)"
        HAS_VALGRIND=0
    fi
    
    if command -v perf >/dev/null 2>&1 && [ "$OS" = "Linux" ]; then
        echo -e "  ${GREEN}✓ perf found${NC} (CPU profiling)"
        HAS_PERF=1
    else
        echo -e "  ${YELLOW}⚠ perf not found${NC} (Linux only, install for CPU profiling)"
        HAS_PERF=0
    fi
    
    # Report missing critical tools
    if [ ${#missing_tools[@]} -ne 0 ]; then
        echo -e "\n${RED}Error: Missing required tools:${NC}"
        for tool in "${missing_tools[@]}"; do
            echo "  - $tool"
        done
        
        echo -e "\n${YELLOW}Installation instructions:${NC}"
        case "$OS" in
            Darwin)
                echo "  brew install git bc"
                echo "  # For Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                echo "  # Optional: brew install hyperfine"
                ;;
            Linux)
                echo "  # Ubuntu/Debian:"
                echo "  sudo apt update && sudo apt install git bc build-essential"
                echo "  # For Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                echo "  # Optional: sudo apt install hyperfine valgrind linux-perf"
                echo ""
                echo "  # RHEL/CentOS/Fedora:"
                echo "  sudo dnf install git bc gcc"
                echo "  # Optional: sudo dnf install hyperfine valgrind perf"
                ;;
        esac
        exit 1
    fi
    
    echo ""
}

# Function to measure memory and performance with OS-specific optimizations
measure_performance() {
    local binary=$1
    local label=$2
    
    # Clear caches if cold run requested
    clear_caches
    
    echo -e "${BLUE}$label:${NC}"
    
    # Binary size analysis
    if [ -f "$binary" ]; then
        local size_bytes=$(stat -f%z "$binary" 2>/dev/null || stat -c%s "$binary" 2>/dev/null)
        local size_human=$(ls -lh "$binary" | awk '{print $5}')
        echo "  Binary size: $size_human ($size_bytes bytes)"
        
        # Strip analysis
        if command -v strip >/dev/null 2>&1; then
            local temp_binary="/tmp/$(basename $binary)_stripped"
            cp "$binary" "$temp_binary"
            strip "$temp_binary" 2>/dev/null
            local stripped_size=$(stat -f%z "$temp_binary" 2>/dev/null || stat -c%s "$temp_binary" 2>/dev/null)
            local savings=$((size_bytes - stripped_size))
            local savings_percent=$(echo "scale=1; ($savings * 100.0) / $size_bytes" | bc -l)
            echo "  Stripped size: $(ls -lh $temp_binary | awk '{print $5}') (saves $savings_percent%)"
            rm -f "$temp_binary"
        fi
    else
        echo -e "  ${RED}Binary not found: $binary${NC}"
        return 1
    fi
    
    # Performance measurement
    echo "  Performance metrics:"
    
    # Test command validity first
    if ! $binary --version >/dev/null 2>&1; then
        echo -e "    ${RED}Error: Binary appears corrupted or incompatible${NC}"
        return 1
    fi
    
    case "$OS" in
        Darwin)
            # macOS: Use time -l for detailed stats
            /usr/bin/time -l $binary check 2>&1 >/dev/null | awk '
            /real/ { 
                gsub(/[^0-9.]/, "", $1)
                print "    Time: " $1 "s" 
            }
            /maximum resident set size/ { 
                mb = int($1/1024/1024)
                print "    Peak Memory: " mb " MB (" $1 " bytes)"
            }
            /voluntary context switches/ { print "    Voluntary ctx switches: " $1 }
            /involuntary context switches/ { print "    Involuntary ctx switches: " $1 }
            /page faults/ { print "    Page faults: " $1 }
            '
            ;;
        Linux)
            # Linux: Use time -v for detailed stats
            /usr/bin/time -v $binary check 2>&1 >/dev/null | awk '
            /Elapsed \(wall clock\) time/ { 
                gsub(/[hms:]/, " ", $8)
                split($8, time_parts)
                if (length(time_parts) == 3) {
                    total = time_parts[1]*3600 + time_parts[2]*60 + time_parts[3]
                } else if (length(time_parts) == 2) {
                    total = time_parts[1]*60 + time_parts[2]
                } else {
                    total = time_parts[1]
                }
                print "    Time: " total "s"
            }
            /Maximum resident set size/ { 
                mb = int($6/1024)
                print "    Peak Memory: " mb " MB (" $6 " KB)"
            }
            /Voluntary context switches/ { 
                if (!voluntary_printed) {
                    print "    Voluntary ctx switches: " $5
                    voluntary_printed = 1
                }
            }
            /Involuntary context switches/ { 
                if (!involuntary_printed) {
                    print "    Involuntary ctx switches: " $5
                    involuntary_printed = 1
                }
            }
            /Major \(requiring I\/O\) page faults/ { print "    Major page faults: " $6 }
            /Minor \(reclaiming a frame\) page faults/ { print "    Minor page faults: " $6 }
            '
            ;;
    esac
    
    # Additional profiling if tools are available
    if [ "$HAS_VALGRIND" = "1" ] && [ "$OS" = "Linux" ]; then
        echo "    Memory profiling (valgrind):"
        valgrind --tool=massif --pages-as-heap=yes --massif-out-file=/tmp/massif.out $binary check >/dev/null 2>&1
        ms_print /tmp/massif.out | grep "peak" | head -1 | awk '{print "      Peak heap: " $3}'
        rm -f /tmp/massif.out
    fi
    
    echo ""
}

# Function to run comprehensive benchmarks
run_benchmarks() {
    if [ "$HAS_HYPERFINE" = "1" ]; then
        echo -e "${BLUE}Comprehensive Benchmark (hyperfine):${NC}"
        echo "Running statistical analysis with warmup..."
        
        if [ -f "./rustowl-baseline" ] && [ -f "./rustowl-current" ]; then
            hyperfine \
                --warmup 2 \
                --min-runs 5 \
                --max-runs 10 \
                --ignore-failure \
                --export-markdown /tmp/benchmark.md \
                --command-name "Baseline" './rustowl-baseline check' \
                --command-name "Current" './rustowl-current check'
            
            if [ -f "/tmp/benchmark.md" ]; then
                echo ""
                echo "Benchmark Results:"
                cat /tmp/benchmark.md
                rm -f /tmp/benchmark.md
            fi
        fi
        echo ""
    fi
}

# Function to analyze binary differences
analyze_binaries() {
    if [ -f "./rustowl-baseline" ] && [ -f "./rustowl-current" ]; then
        echo -e "${BLUE}Binary Analysis:${NC}"
        
        # Size comparison
        baseline_size=$(stat -f%z ./rustowl-baseline 2>/dev/null || stat -c%s ./rustowl-baseline 2>/dev/null)
        current_size=$(stat -f%z ./rustowl-current 2>/dev/null || stat -c%s ./rustowl-current 2>/dev/null)
        
        size_diff=$((current_size - baseline_size))
        size_percent=$(echo "scale=2; ($size_diff * 100.0) / $baseline_size" | bc -l)
        
        echo "Size comparison:"
        echo "  Baseline: $(ls -lh ./rustowl-baseline | awk '{print $5}') ($baseline_size bytes)"
        echo "  Current:  $(ls -lh ./rustowl-current | awk '{print $5}') ($current_size bytes)"
        echo "  Difference: $size_diff bytes ($size_percent%)"
        
        if [ "$size_diff" -gt 0 ]; then
            echo -e "  ${YELLOW}⚠ Binary size increased${NC}"
        elif [ "$size_diff" -lt 0 ]; then
            echo -e "  ${GREEN}✓ Binary size decreased${NC}"
        else
            echo -e "  ${BLUE}→ Binary size unchanged${NC}"
        fi
        
        # Symbol analysis if nm is available
        if command -v nm >/dev/null 2>&1; then
            echo ""
            echo "Symbol analysis:"
            baseline_symbols=$(nm ./rustowl-baseline 2>/dev/null | wc -l)
            current_symbols=$(nm ./rustowl-current 2>/dev/null | wc -l)
            echo "  Baseline symbols: $baseline_symbols"
            echo "  Current symbols:  $current_symbols"
            echo "  Symbol difference: $((current_symbols - baseline_symbols))"
        fi
        
        echo ""
    fi
}

# Function to verify environment and show what will be tested
verify_environment() {
    echo -e "${BLUE}Environment Verification${NC}"
    echo "======================="
    
    # Git status
    echo "Git repository status:"
    echo "  Current branch: $(git branch --show-current 2>/dev/null || echo 'Not a git repo')"
    echo "  Current commit: $(git log --oneline -n 1 2>/dev/null || echo 'No commits')"
    
    # Check for uncommitted changes
    if git diff-index --quiet HEAD -- 2>/dev/null; then
        if git ls-files --others --exclude-standard | grep -q . 2>/dev/null; then
            echo -e "  ${YELLOW}Status: Untracked files present${NC}"
        else
            echo -e "  ${YELLOW}Status: No uncommitted changes (binaries will be identical)${NC}"
        fi
    else
        echo -e "  ${GREEN}Status: Uncommitted changes detected${NC}"
    fi
    
    # Check existing binaries
    echo ""
    echo "Existing test binaries:"
    if [ -f "./rustowl-baseline" ]; then
        baseline_hash=$(shasum -a 256 ./rustowl-baseline | cut -d' ' -f1)
        echo "  rustowl-baseline: present (SHA256: ${baseline_hash:0:16}...)"
    else
        echo "  rustowl-baseline: not found"
    fi
    
    if [ -f "./rustowl-current" ]; then
        current_hash=$(shasum -a 256 ./rustowl-current | cut -d' ' -f1)
        echo "  rustowl-current: present (SHA256: ${current_hash:0:16}...)"
    else
        echo "  rustowl-current: not found"
    fi
    
    if [ -f "./rustowl-baseline" ] && [ -f "./rustowl-current" ]; then
        if [ "$baseline_hash" = "$current_hash" ]; then
            echo -e "  ${YELLOW}⚠ WARNING: Existing binaries are identical${NC}"
        else
            echo -e "  ${GREEN}✓ Existing binaries are different${NC}"
        fi
    fi
    
    # Show current build artifacts
    echo ""
    echo "Current build artifacts:"
    if [ -f "target/release/rustowl" ]; then
        current_size=$(stat -f%z "target/release/rustowl" 2>/dev/null || stat -c%s "target/release/rustowl" 2>/dev/null)
        echo "  target/release/rustowl: $(ls -lh target/release/rustowl | awk '{print $5}') ($current_size bytes)"
    else
        echo "  target/release/rustowl: not found"
    fi
    
    # Tool availability
    echo ""
    echo "Available performance tools:"
    check_tools_quiet
    
    echo ""
}

# Quiet version of check_tools for verification
check_tools_quiet() {
    command -v git >/dev/null 2>&1 && echo "  ✓ git" || echo "  ✗ git"
    command -v cargo >/dev/null 2>&1 && echo "  ✓ cargo" || echo "  ✗ cargo"
    command -v bc >/dev/null 2>&1 && echo "  ✓ bc" || echo "  ✗ bc"
    command -v hyperfine >/dev/null 2>&1 && echo "  ✓ hyperfine" || echo "  ✗ hyperfine"
    command -v valgrind >/dev/null 2>&1 && echo "  ✓ valgrind" || echo "  ✗ valgrind"
    [ "$OS" = "Darwin" ] && echo "  ✓ time (macOS)" || echo "  ✓ time (Linux)"
}

# Function to prepare test environment (build both versions)
prepare_test_environment() {
    echo -e "${BLUE}Preparing Test Environment${NC}"
    echo "=========================="
    
    # Check if binaries already exist and if force rebuild is needed
    if [ -f "./rustowl-baseline" ] && [ -f "./rustowl-current" ] && [ $FORCE_REBUILD -eq 0 ]; then
        echo -e "${YELLOW}Test binaries already exist. Use --force to rebuild.${NC}"
        echo "Use --verify to inspect existing binaries or --compare to test them."
        return 0
    fi
    
    # Clean up existing binaries if force rebuild
    if [ $FORCE_REBUILD -eq 1 ]; then
        echo "Force rebuild requested, cleaning up existing binaries..."
        rm -f ./rustowl-baseline ./rustowl-current ./rustowlc-baseline ./rustowlc-current
    fi
    
    # Save current state
    echo -e "${YELLOW}Saving current changes...${NC}"
    
    # Check if there are any changes to stash
    if git diff-index --quiet HEAD --; then
        if git ls-files --others --exclude-standard | grep -q .; then
            echo "Found untracked files, stashing..."
            git stash push -m "Performance test stash $(date)" --include-untracked
            STASH_CREATED=$?
        else
            echo "No changes to stash."
            STASH_CREATED=1
        fi
    else
        echo "Found changes, stashing..."
        git stash push -m "Performance test stash $(date)" --include-untracked
        STASH_CREATED=$?
    fi
    
    echo "Current git state after stash:"
    git log --oneline -n 1
    git status --porcelain
    
    # Build baseline version
    echo -e "${YELLOW}Building baseline version...${NC}"
    cargo clean --quiet
    cargo build --release --quiet
    if [ $? -eq 0 ]; then
        cp target/release/rustowl ./rustowl-baseline
        [ -f target/release/rustowlc ] && cp target/release/rustowlc ./rustowlc-baseline
        echo -e "${GREEN}✓ Baseline binaries created${NC}"
    else
        echo -e "${RED}✗ Failed to build baseline${NC}"
        exit 1
    fi
    
    # Restore working changes
    if [ $STASH_CREATED -eq 0 ]; then
        echo -e "${YELLOW}Restoring current changes...${NC}"
        git stash pop --quiet
        echo "Git state after restoring changes:"
        git log --oneline -n 1
        git status --porcelain
    else
        echo "No stash to restore."
    fi
    
    # Build current version
    echo -e "${YELLOW}Building current version...${NC}"
    cargo clean --quiet
    cargo build --release --quiet
    if [ $? -eq 0 ]; then
        cp target/release/rustowl ./rustowl-current
        [ -f target/release/rustowlc ] && cp target/release/rustowlc ./rustowlc-current
        echo -e "${GREEN}✓ Current binaries created${NC}"
    else
        echo -e "${RED}✗ Failed to build current version${NC}"
        exit 1
    fi
    
    # Verify the prepared binaries
    echo ""
    echo -e "${GREEN}✓ Test environment prepared successfully${NC}"
    verify_prepared_binaries
}

# Function to verify prepared binaries
verify_prepared_binaries() {
    echo ""
    echo "Prepared binaries verification:"
    
    if [ -f "./rustowl-baseline" ] && [ -f "./rustowl-current" ]; then
        baseline_hash=$(shasum -a 256 ./rustowl-baseline | cut -d' ' -f1)
        current_hash=$(shasum -a 256 ./rustowl-current | cut -d' ' -f1)
        
        echo "  Baseline SHA256: $baseline_hash"
        echo "  Current SHA256:  $current_hash"
        
        if [ "$baseline_hash" = "$current_hash" ]; then
            echo -e "  ${YELLOW}⚠ WARNING: Binaries are identical!${NC}"
            echo "    This indicates no code changes between versions."
            echo "    Performance comparison will show measurement noise only."
        else
            echo -e "  ${GREEN}✓ Binaries are different - valid for comparison${NC}"
        fi
        
        # Test that binaries work
        echo ""
        echo "Testing binary functionality:"
        if ./rustowl-baseline --version >/dev/null 2>&1; then
            echo -e "  ${GREEN}✓ Baseline binary functional${NC}"
        else
            echo -e "  ${RED}✗ Baseline binary failed${NC}"
        fi
        
        if ./rustowl-current --version >/dev/null 2>&1; then
            echo -e "  ${GREEN}✓ Current binary functional${NC}"
        else
            echo -e "  ${RED}✗ Current binary failed${NC}"
        fi
    else
        echo -e "  ${RED}✗ Missing binaries - run --prepare first${NC}"
        return 1
    fi
}

# Function to compare prepared binaries
compare_prepared_binaries() {
    echo -e "${BLUE}Comparing Prepared Binaries${NC}"
    echo "=========================="
    
    # Check if binaries exist
    if [ ! -f "./rustowl-baseline" ] || [ ! -f "./rustowl-current" ]; then
        echo -e "${RED}✗ Test binaries not found. Run with --prepare first.${NC}"
        exit 1
    fi
    
    # Verify binaries first
    verify_prepared_binaries
    echo ""
    
    # Run performance comparison
    echo -e "${BLUE}Performance Comparison${NC}"
    echo "====================="
    
    # Measure both versions
    measure_performance "./rustowl-baseline" "Baseline (stashed version)"
    measure_performance "./rustowl-current" "Current (working version)"
    
    # Binary analysis
    analyze_binaries
    
    # Advanced benchmarks
    run_benchmarks
    
    # Cleanup
    echo -e "${YELLOW}Cleaning up temporary files...${NC}"
    rm -f ./rustowl-baseline ./rustowl-current ./rustowlc-baseline ./rustowlc-current
    
    # Summary
    echo -e "${BLUE}Summary & Recommendations:${NC}"
    echo "========================="
    echo "• If you saw identical binaries, ensure you have uncommitted changes"
    echo "• Large performance differences may indicate:"
    echo "  - Caching effects (run the test multiple times)"
    echo "  - Different test conditions (background processes, thermal throttling)"
    echo "  - Measurement artifacts (especially if page faults = 0)"
    echo "• For more detailed profiling, consider:"
    case "$OS" in
        Darwin)
            echo "  - instruments -t 'Time Profiler' ./rustowl check"
            echo "  - leaks --atExit -- ./rustowl check"
            ;;
        Linux)
            echo "  - valgrind --tool=callgrind ./rustowl check"
            echo "  - perf record ./rustowl check && perf report"
            ;;
    esac
    
    echo ""
}

# Function to run multiple test iterations to detect inconsistencies
run_stability_test() {
    local binary=$1
    local label=$2
    local iterations=3
    
    echo -e "${BLUE}Stability Test: $label${NC}"
    if [ $COLD_RUN -eq 1 ]; then
        echo "Running $iterations cold iterations (cache cleared before each)..."
    else
        echo "Running $iterations warm iterations to check for consistency..."
    fi
    
    local times=()
    local memories=()
    
    for i in $(seq 1 $iterations); do
        echo "  Iteration $i..."
        
        # Clear caches for cold runs
        if [ $COLD_RUN -eq 1 ]; then
            clear_caches
        fi
        case "$OS" in
            Darwin)
                local result=$(/usr/bin/time -l $binary check 2>&1 >/dev/null)
                local time_val=$(echo "$result" | awk '/real/ { gsub(/[^0-9.]/, "", $1); print $1 }')
                local mem_val=$(echo "$result" | awk '/maximum resident set size/ { print int($1/1024/1024) }')
                ;;
            Linux)
                local result=$(/usr/bin/time -v $binary check 2>&1 >/dev/null)
                local time_val=$(echo "$result" | awk '/Elapsed \(wall clock\) time/ { 
                    gsub(/[hms:]/, " ", $8)
                    split($8, time_parts)
                    if (length(time_parts) == 3) {
                        print time_parts[1]*3600 + time_parts[2]*60 + time_parts[3]
                    } else if (length(time_parts) == 2) {
                        print time_parts[1]*60 + time_parts[2]
                    } else {
                        print time_parts[1]
                    }
                }')
                local mem_val=$(echo "$result" | awk '/Maximum resident set size/ { print int($6/1024) }')
                ;;
        esac
        
        times+=($time_val)
        memories+=($mem_val)
        echo "    Time: ${time_val}s, Memory: ${mem_val}MB"
    done
    
    # Calculate statistics
    echo "  Results summary:"
    local time_min=$(printf '%s\n' "${times[@]}" | sort -n | head -1)
    local time_max=$(printf '%s\n' "${times[@]}" | sort -n | tail -1)
    local mem_min=$(printf '%s\n' "${memories[@]}" | sort -n | head -1)
    local mem_max=$(printf '%s\n' "${memories[@]}" | sort -n | tail -1)
    
    echo "    Time range: ${time_min}s - ${time_max}s"
    echo "    Memory range: ${mem_min}MB - ${mem_max}MB"
    
    # Check for suspicious variation
    local time_ratio=$(echo "scale=2; $time_max / $time_min" | bc -l)
    local mem_ratio=$(echo "scale=2; $mem_max / $mem_min" | bc -l)
    
    if (( $(echo "$time_ratio > 2.0" | bc -l) )); then
        echo -e "    ${YELLOW}⚠ High time variation (${time_ratio}x) - results may be unreliable${NC}"
    fi
    
    if (( $(echo "$mem_ratio > 2.0" | bc -l) )); then
        echo -e "    ${YELLOW}⚠ High memory variation (${mem_ratio}x) - results may be unreliable${NC}"
    fi
    
    echo ""
}

# Enhanced comparison with stability testing
compare_with_stability_check() {
    echo -e "${BLUE}Enhanced Performance Comparison with Stability Check${NC}"
    echo "=================================================="
    
    # Check if binaries exist
    if [ ! -f "./rustowl-baseline" ] || [ ! -f "./rustowl-current" ]; then
        echo -e "${RED}✗ Test binaries not found. Run with --prepare first.${NC}"
        exit 1
    fi
    
    # Verify binaries first
    verify_prepared_binaries
    echo ""
    
    # Run stability tests
    run_stability_test "./rustowl-baseline" "Baseline"
    run_stability_test "./rustowl-current" "Current"
    
    # Run standard comparison
    echo -e "${BLUE}Single Measurement Comparison${NC}"
    echo "============================"
    measure_performance "./rustowl-baseline" "Baseline (stashed version)"
    measure_performance "./rustowl-current" "Current (working version)"
    
    # Binary analysis
    analyze_binaries
    
    # Advanced benchmarks if available
    run_benchmarks
}

# Function to clear caches for cold runs
clear_caches() {
    if [ $COLD_RUN -eq 0 ]; then
        return 0
    fi
    
    echo -e "${YELLOW}Clearing caches for cold run...${NC}"
    
    # Clear Rust build cache
    echo "  Clearing Rust build artifacts..."
    cargo clean --quiet
    
    # Kill any running instances
    echo "  Stopping any running rustowl processes..."
    killall -9 rustowl rustowlc 2>/dev/null || true
    
    # OS-specific cache clearing
    case "$OS" in
        Darwin)
            echo "  Clearing macOS filesystem cache..."
            if command -v sudo >/dev/null 2>&1; then
                sudo purge 2>/dev/null || {
                    echo -e "    ${YELLOW}Warning: Could not clear system cache (requires admin)${NC}"
                    echo "    Run 'sudo purge' manually for complete cache clearing"
                }
            else
                echo -e "    ${YELLOW}Warning: sudo not available, cannot clear system cache${NC}"
            fi
            ;;
        Linux)
            echo "  Clearing Linux filesystem cache..."
            if command -v sudo >/dev/null 2>&1; then
                sync
                echo 3 | sudo tee /proc/sys/vm/drop_caches >/dev/null 2>&1 || {
                    echo -e "    ${YELLOW}Warning: Could not clear system cache (requires root)${NC}"
                    echo "    Run 'sudo sync && echo 3 | sudo tee /proc/sys/vm/drop_caches' manually"
                }
                # Clear shared library cache
                sudo ldconfig 2>/dev/null || true
            else
                echo -e "    ${YELLOW}Warning: sudo not available, cannot clear system cache${NC}"
            fi
            ;;
    esac
    
    # Wait a moment for caches to clear
    echo "  Waiting for cache clearing to complete..."
    sleep 2
    
    echo -e "${GREEN}✓ Cache clearing completed${NC}"
    echo ""
}

# Main execution
main() {
    check_tools
    
    case "$MODE" in
        full)
            echo -e "${BLUE}Running full performance test...${NC}"
            prepare_test_environment
            echo ""
            compare_prepared_binaries
            ;;
        prepare)
            echo -e "${BLUE}Preparing test environment only...${NC}"
            prepare_test_environment
            echo ""
            echo -e "${GREEN}✓ Preparation complete. Use --compare or --stability to run tests.${NC}"
            ;;
        compare)
            echo -e "${BLUE}Comparing pre-built binaries...${NC}"
            compare_prepared_binaries
            ;;
        stability)
            echo -e "${BLUE}Running stability test with multiple iterations...${NC}"
            compare_with_stability_check
            ;;
        verify)
            echo -e "${BLUE}Verifying environment...${NC}"
            verify_environment
            ;;
        *)
            echo "Unknown mode: $MODE"
            usage
            exit 1
            ;;
    esac
}

# Run main function
main "$@"
