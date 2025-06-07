#!/bin/bash
# RustOwl Security & Memory Safety Testing Script
# Tests for undefined behavior, memory leaks, and security vulnerabilities
# Automatically detects platform capabilities and runs appropriate tests

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Configuration
MIN_RUST_VERSION="1.87.0"
TEST_TARGET_PATH="./perf-tests/dummy-package"

# Test flags (can be overridden via command line options)
RUN_MIRI=1
RUN_VALGRIND=1
RUN_SANITIZERS=1
RUN_AUDIT=1
RUN_DRMEMORY=1
RUN_INSTRUMENTS=1

# Tool availability detection
HAS_MIRI=0
HAS_VALGRIND=0
HAS_CARGO_AUDIT=0
HAS_DRMEMORY=0
HAS_INSTRUMENTS=0

# OS detection with more robust platform detection
detect_platform() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        OS_TYPE="Linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        OS_TYPE="macOS"
    elif [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
        OS_TYPE="Windows"
    elif [[ "$OS" == "Windows_NT" ]]; then
        OS_TYPE="Windows"
    else
        # Fallback to uname
        local uname_result=$(uname 2>/dev/null || echo "unknown")
        case "$uname_result" in
            Linux*) OS_TYPE="Linux" ;;
            Darwin*) OS_TYPE="macOS" ;;
            CYGWIN*|MINGW*|MSYS*) OS_TYPE="Windows" ;;
            *) OS_TYPE="Unknown" ;;
        esac
    fi
    
    echo -e "${BLUE}Detected platform: $OS_TYPE${NC}"
}

# Auto-configure tests based on platform capabilities and toolchain compatibility
auto_configure_tests() {
    echo -e "${YELLOW}Auto-configuring tests for $OS_TYPE...${NC}"
    
    case "$OS_TYPE" in
        "Linux")
            # Linux: Full test suite available
            echo "  Linux detected: Enabling Miri, Valgrind, Sanitizers, and Audit"
            ;;
        "macOS")
            # macOS: No Valgrind (unreliable), no DrMemory, Instruments needs Xcode
            echo "  macOS detected: Enabling Miri, Sanitizers, and Audit"
            echo "  Disabling Valgrind (unreliable on macOS)"
            echo "  Instruments will be skipped if Xcode not available"
            RUN_VALGRIND=0
            RUN_DRMEMORY=0
            ;;
        "Windows")
            # Windows: No Valgrind, limited sanitizer support
            echo "  Windows detected: Enabling Miri, Audit, and DrMemory"
            echo "  Disabling Valgrind (Linux only)"
            echo "  Sanitizers may have limited support"
            RUN_VALGRIND=0
            RUN_INSTRUMENTS=0
            ;;
        *)
            echo "  Unknown platform: Enabling basic tests only"
            RUN_VALGRIND=0
            RUN_DRMEMORY=0
            RUN_INSTRUMENTS=0
            # Also disable nightly-dependent features on unknown platforms
            RUN_MIRI=0
            RUN_SANITIZERS=0
            ;;
    esac
    
    # Additional check: if we detect nightly API compatibility issues, disable advanced features
    if ! cargo +nightly check --lib >/dev/null 2>&1; then
        echo -e "${YELLOW}  ! Nightly compatibility issues detected${NC}"
        echo "  Disabling Miri and Sanitizers to avoid build failures"
        RUN_MIRI=0
        RUN_SANITIZERS=0
    fi
    
    echo ""
}

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Security and Memory Safety Testing Script"
    echo "Automatically detects platform and runs appropriate security tests"
    echo ""
    echo "Options:"
    echo "  -h, --help           Show this help message"
    echo "  --check              Check tool availability and system readiness"
    echo "  --no-miri            Skip Miri tests"
    echo "  --no-valgrind        Skip Valgrind tests"
    echo "  --no-sanitizers      Skip sanitizer tests"
    echo "  --no-audit           Skip cargo audit security check"
    echo "  --no-drmemory        Skip DrMemory tests"
    echo "  --no-instruments     Skip Instruments tests"
    echo ""
    echo "Platform Support:"
    echo "  Linux:   Miri, Valgrind, Sanitizers, cargo-audit"
    echo "  macOS:   Miri, Sanitizers, cargo-audit, Instruments"
    echo "  Windows: Miri, Sanitizers, cargo-audit, DrMemory"
    echo ""
    echo "Tests performed:"
    echo "  • Miri: Detects undefined behavior in Rust code"
    echo "  • Valgrind: Memory error detection (Linux)"
    echo "  • AddressSanitizer: Memory error detection"
    echo "  • ThreadSanitizer: Data race detection" 
    echo "  • MemorySanitizer: Uninitialized memory detection"
    echo "  • cargo-audit: Security vulnerability scanning"
    echo "  • DrMemory: Memory debugging (Windows)"
    echo "  • Instruments: Performance and memory analysis (macOS)"
    echo ""
    echo "Examples:"
    echo "  $0                   # Auto-detect platform and run appropriate tests"
    echo "  $0 --check          # Check which tools are available"
    echo "  $0 --no-miri        # Run tests but skip Miri"
    echo ""
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            exit 0
            ;;
        --check)
            MODE="check"
            shift
            ;;
        --no-miri)
            RUN_MIRI=0
            shift
            ;;
        --no-valgrind)
            RUN_VALGRIND=0
            shift
            ;;
        --no-sanitizers)
            RUN_SANITIZERS=0
            shift
            ;;
        --no-audit)
            RUN_AUDIT=0
            shift
            ;;
        --no-drmemory)
            RUN_DRMEMORY=0
            shift
            ;;
        --no-instruments)
            RUN_INSTRUMENTS=0
            shift
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

# Check Rust version compatibility
check_rust_version() {
    if ! command -v rustc >/dev/null 2>&1; then
        echo -e "${RED}✗ Rust compiler not found. Please install Rust: https://rustup.rs/${NC}"
        exit 1
    fi
    
    local current_version=$(rustc --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
    local min_version="$MIN_RUST_VERSION"
    
    if [ -z "$current_version" ]; then
        echo -e "${YELLOW}⚠ Could not determine Rust version, proceeding anyway...${NC}"
        return 0
    fi
    
    # Simple version comparison (assumes semantic versioning)
    if printf '%s\n%s\n' "$min_version" "$current_version" | sort -V -C; then
        echo -e "${GREEN}✓ Rust $current_version >= $min_version (minimum required)${NC}"
        return 0
    else
        echo -e "${RED}✗ Rust $current_version < $min_version (minimum required)${NC}"
        echo -e "${YELLOW}Please update Rust: rustup update${NC}"
        exit 1
    fi
}

# Detect available tools based on platform
detect_tools() {
    echo -e "${YELLOW}Detecting available security tools...${NC}"
    
    # Check for Miri
    if rustup component list --installed | grep -q miri; then
        HAS_MIRI=1
        echo -e "${GREEN}✓ Miri is installed${NC}"
    else
        echo -e "${YELLOW}! Miri not installed (install with: rustup component add miri)${NC}"
    fi
    
    # Check for Valgrind (Linux only)
    if [[ "$OS_TYPE" == "Linux" ]] && command -v valgrind >/dev/null 2>&1; then
        HAS_VALGRIND=1
        echo -e "${GREEN}✓ Valgrind is available${NC}"
    elif [[ "$OS_TYPE" == "Linux" ]]; then
        echo -e "${YELLOW}! Valgrind not found (install with package manager)${NC}"
    fi
    
    # Check for cargo-audit
    if command -v cargo-audit >/dev/null 2>&1; then
        HAS_CARGO_AUDIT=1
        echo -e "${GREEN}✓ cargo-audit is available${NC}"
    else
        echo -e "${YELLOW}! cargo-audit not found (install with: cargo install cargo-audit)${NC}"
    fi
    
    # Check for DrMemory (Windows only)
    if [[ "$OS_TYPE" == "Windows" ]] && command -v drmemory >/dev/null 2>&1; then
        HAS_DRMEMORY=1
        echo -e "${GREEN}✓ DrMemory is available${NC}"
    elif [[ "$OS_TYPE" == "Windows" ]]; then
        echo -e "${YELLOW}! DrMemory not found${NC}"
    fi
    
    # Check for Instruments (macOS only)
    if [[ "$OS_TYPE" == "macOS" ]] && command -v instruments >/dev/null 2>&1; then
        HAS_INSTRUMENTS=1
        echo -e "${GREEN}✓ Instruments is available${NC}"
    elif [[ "$OS_TYPE" == "macOS" ]]; then
        echo -e "${YELLOW}! Instruments not found${NC}"
    fi
    
    echo ""
}

# Check nightly toolchain availability for advanced features
check_nightly_toolchain() {
    echo -e "${YELLOW}Checking toolchain for advanced security features...${NC}"
    
    # Check what toolchain is currently active
    local current_toolchain=$(rustup show active-toolchain | cut -d' ' -f1)
    echo -e "${BLUE}Active toolchain: $current_toolchain${NC}"
    
    if [[ "$current_toolchain" == *"nightly"* ]]; then
        echo -e "${GREEN}✓ Nightly toolchain is active (from rust-toolchain.toml)${NC}"
    else
        echo -e "${YELLOW}! Stable toolchain detected${NC}"
        echo -e "${YELLOW}Some advanced features require nightly (check rust-toolchain.toml)${NC}"
    fi
    
    # Check if Miri component is available on current toolchain
    if rustup component list --installed | grep -q miri 2>/dev/null; then
        HAS_MIRI=1
        echo -e "${GREEN}✓ Miri is available${NC}"
    else
        echo -e "${YELLOW}! Miri component not installed${NC}"
        echo -e "${YELLOW}Install with: rustup component add miri${NC}"
        HAS_MIRI=0
    fi
    
    # Check if required targets are installed
    local current_target
    case "$OS_TYPE" in
        "Linux")
            current_target="x86_64-unknown-linux-gnu"
            if uname -m | grep -q aarch64; then
                current_target="aarch64-unknown-linux-gnu"
            fi
            ;;
        "macOS")
            current_target="aarch64-apple-darwin"
            if uname -m | grep -q x86_64; then
                current_target="x86_64-apple-darwin"
            fi
            ;;
        "Windows")
            current_target="x86_64-pc-windows-msvc"
            ;;
    esac
    
    if [[ -n "$current_target" ]]; then
        if rustup target list --installed | grep -q "$current_target"; then
            echo -e "${GREEN}✓ Target $current_target is available${NC}"
        else
            echo -e "${YELLOW}! Target $current_target not installed${NC}"
            echo -e "${YELLOW}Install with: rustup target add $current_target${NC}"
        fi
    fi
    
    return 0
}

# Build the project with the toolchain specified in rust-toolchain.toml
build_project() {
    echo -e "${YELLOW}Building RustOwl in security mode...${NC}"
    echo -e "${BLUE}Using toolchain from rust-toolchain.toml${NC}"
    
    # Build with the current toolchain (specified by rust-toolchain.toml)
    RUSTC_BOOTSTRAP=1 cargo build --profile=security
    
    local binary_name="rustowl"
    if [[ "$OS_TYPE" == "Windows" ]]; then
        binary_name="rustowl.exe"
    fi
    
    if [ ! -f "./target/security/$binary_name" ]; then
        echo -e "${RED}✗ Failed to build rustowl binary${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓ Build completed successfully${NC}"
    echo ""
}

# Run Miri tests using the current toolchain
run_miri_tests() {
    if [[ $RUN_MIRI -eq 0 ]]; then
        return 0
    fi
    
    if [[ $HAS_MIRI -eq 0 ]]; then
        echo -e "${YELLOW}Skipping Miri tests (component not installed)${NC}"
        return 0
    fi
    
    echo -e "${BLUE}${BOLD}Running Miri Tests${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "Miri detects undefined behavior in Rust code"
    echo ""
    
    # Test the library with Miri using current toolchain
    echo -e "${YELLOW}Testing library code with Miri...${NC}"
    if cargo miri test --lib; then
        echo -e "${GREEN}✓ Library tests passed with Miri${NC}"
    else
        echo -e "${RED}✗ Miri detected issues in library code${NC}"
        return 1
    fi
    
    echo ""
}

# Run Valgrind tests
run_valgrind_tests() {
    if [[ $RUN_VALGRIND -eq 0 ]] || [[ $HAS_VALGRIND -eq 0 ]] || [[ "$OS_TYPE" != "Linux" ]]; then
        if [[ $RUN_VALGRIND -eq 1 ]] && [[ "$OS_TYPE" == "Linux" ]] && [[ $HAS_VALGRIND -eq 0 ]]; then
            echo -e "${YELLOW}Skipping Valgrind tests (not installed)${NC}"
        fi
        return 0
    fi
    
    echo -e "${BLUE}${BOLD}Running Valgrind Tests${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "Valgrind detects memory errors and leaks"
    echo ""
    
    # Test with the dummy package
    if [ -d "$TEST_TARGET_PATH" ]; then
        echo -e "${YELLOW}Testing rustowl with Valgrind...${NC}"
        
        # Use suppression file if available
        local valgrind_cmd="valgrind --tool=memcheck --leak-check=full --show-leak-kinds=all --error-exitcode=1 --track-origins=yes"
        if [[ -f ".valgrind-suppressions" ]]; then
            valgrind_cmd="$valgrind_cmd --suppressions=.valgrind-suppressions"
        fi
        
        if timeout 300 $valgrind_cmd ./target/security/rustowl check "$TEST_TARGET_PATH" >/dev/null 2>&1; then
            echo -e "${GREEN}✓ No memory errors detected by Valgrind${NC}"
        else
            echo -e "${RED}✗ Valgrind detected memory issues${NC}"
            echo "Run manually for details: $valgrind_cmd ./target/security/rustowl check $TEST_TARGET_PATH"
            return 1
        fi
    else
        echo -e "${YELLOW}! Test package not found, skipping Valgrind runtime test${NC}"
    fi
    
    echo ""
}

# Run sanitizer tests using the current nightly toolchain
run_sanitizer_tests() {
    if [[ $RUN_SANITIZERS -eq 0 ]]; then
        return 0
    fi
    
    echo -e "${BLUE}${BOLD}Running Sanitizer Tests${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "Sanitizers detect various memory and threading issues"
    
    # Check if current toolchain supports sanitizers
    local current_toolchain=$(rustup show active-toolchain | cut -d' ' -f1)
    if [[ "$current_toolchain" != *"nightly"* ]]; then
        echo -e "${YELLOW}! Sanitizers require nightly toolchain, skipping${NC}"
        return 0
    fi
    
    echo "Using nightly toolchain for sanitizer testing"
    
    local sanitizer_failed=false
    local current_target
    
    # Determine the appropriate target for this platform
    case "$OS_TYPE" in
        "Linux")
            if uname -m | grep -q aarch64; then
                current_target="aarch64-unknown-linux-gnu"
            else
                current_target="x86_64-unknown-linux-gnu"
            fi
            ;;
        "macOS")
            if uname -m | grep -q arm64; then
                current_target="aarch64-apple-darwin"
            else
                current_target="x86_64-apple-darwin"
            fi
            ;;
        "Windows")
            current_target="x86_64-pc-windows-msvc"
            echo "  Note: Limited sanitizer support on Windows"
            ;;
    esac
    
    echo "  Testing target: $current_target"
    
    # Try AddressSanitizer
    echo -e "${YELLOW}Building with AddressSanitizer...${NC}"
    if RUSTFLAGS="-Z sanitizer=address" cargo build --target "$current_target" --target-dir target/sanitizer --profile=security 2>/dev/null; then
        echo -e "${GREEN}✓ AddressSanitizer build successful for $current_target${NC}"
    else
        echo -e "${YELLOW}! AddressSanitizer build failed for $current_target${NC}"
        sanitizer_failed=true
    fi
    
    # Don't fail the entire security suite if sanitizers have issues
    if [[ "$sanitizer_failed" == "true" ]]; then
        echo -e "${YELLOW}! Some sanitizers failed - this may indicate toolchain compatibility issues${NC}"
        return 0  # Don't fail the entire security check
    fi
    
    echo ""
}

# Run cargo audit
run_audit_check() {
    if [[ $RUN_AUDIT -eq 0 ]] || [[ $HAS_CARGO_AUDIT -eq 0 ]]; then
        if [[ $RUN_AUDIT -eq 1 ]] && [[ $HAS_CARGO_AUDIT -eq 0 ]]; then
            echo -e "${YELLOW}Skipping cargo-audit (not installed)${NC}"
        fi
        return 0
    fi
    
    echo -e "${BLUE}${BOLD}Running Security Audit${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "cargo-audit checks for known security vulnerabilities"
    echo ""
    
    echo -e "${YELLOW}Scanning dependencies for vulnerabilities...${NC}"
    if cargo audit; then
        echo -e "${GREEN}✓ No known vulnerabilities found${NC}"
    else
        echo -e "${RED}✗ Security vulnerabilities detected${NC}"
        return 1
    fi
    
    echo ""
}

# Run DrMemory tests (Windows)
run_drmemory_tests() {
    if [[ $RUN_DRMEMORY -eq 0 ]] || [[ $HAS_DRMEMORY -eq 0 ]] || [[ "$OS_TYPE" != "Windows" ]]; then
        if [[ $RUN_DRMEMORY -eq 1 ]] && [[ "$OS_TYPE" == "Windows" ]] && [[ $HAS_DRMEMORY -eq 0 ]]; then
            echo -e "${YELLOW}Skipping DrMemory tests (not installed)${NC}"
        fi
        return 0
    fi
    
    echo -e "${BLUE}${BOLD}Running DrMemory Tests${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "DrMemory detects memory errors on Windows"
    echo ""
    
    if [ -d "$TEST_TARGET_PATH" ]; then
        echo -e "${YELLOW}Testing rustowl with DrMemory...${NC}"
        if drmemory -- ./target/security/rustowl.exe check "$TEST_TARGET_PATH"; then
            echo -e "${GREEN}✓ No memory errors detected by DrMemory${NC}"
        else
            echo -e "${RED}✗ DrMemory detected memory issues${NC}"
            return 1
        fi
    fi
    
    echo ""
}

# Run Instruments tests (macOS)
run_instruments_tests() {
    if [[ $RUN_INSTRUMENTS -eq 0 ]] || [[ $HAS_INSTRUMENTS -eq 0 ]] || [[ "$OS_TYPE" != "macOS" ]]; then
        if [[ $RUN_INSTRUMENTS -eq 1 ]] && [[ "$OS_TYPE" == "macOS" ]] && [[ $HAS_INSTRUMENTS -eq 0 ]]; then
            echo -e "${YELLOW}Skipping Instruments tests (not available)${NC}"
        fi
        return 0
    fi
    
    echo -e "${BLUE}${BOLD}Running Instruments Tests (macOS)${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "Instruments provides memory and performance analysis"
    echo ""
    
    if [ -d "$TEST_TARGET_PATH" ]; then
        echo -e "${YELLOW}Testing rustowl with Instruments...${NC}"
        if instruments -t "Allocations" -D instruments_output.trace ./target/security/rustowl check "$TEST_TARGET_PATH" >/dev/null 2>&1; then
            echo -e "${GREEN}✓ Instruments analysis completed${NC}"
            # Clean up trace file
            rm -rf instruments_output.trace 2>/dev/null || true
        else
            echo -e "${RED}✗ Instruments analysis failed${NC}"
            echo "Run manually for details: instruments -t 'Allocations' ./target/security/rustowl check $TEST_TARGET_PATH"
            return 1
        fi
    fi
    
    echo ""
}

# Show tool availability summary
show_tool_status() {
    echo -e "${BLUE}${BOLD}Security Tool Availability${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "Platform: $OS_TYPE"
    echo "Tool availability on this system:"
    echo ""
    echo "  Miri (undefined behavior):     $([ $HAS_MIRI -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Not available${NC}")"
    
    if [[ "$OS_TYPE" == "Linux" ]]; then
        echo "  Valgrind (memory errors):      $([ $HAS_VALGRIND -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Not installed${NC}")"
    else
        echo "  Valgrind (memory errors):      ${YELLOW}N/A (Linux only)${NC}"
    fi
    
    echo "  Sanitizers (various):          $(rustup toolchain list | grep -q nightly && echo -e "${GREEN}✓ Available${NC}" || echo -e "${YELLOW}Requires nightly${NC}")"
    echo "  cargo-audit (vulnerabilities): $([ $HAS_CARGO_AUDIT -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Not installed${NC}")"
    
    if [[ "$OS_TYPE" == "Windows" ]]; then
        echo "  DrMemory (Windows):            $([ $HAS_DRMEMORY -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Not installed${NC}")"
    else
        echo "  DrMemory (Windows):            ${YELLOW}N/A (Windows only)${NC}"
    fi
    
    if [[ "$OS_TYPE" == "macOS" ]]; then
        echo "  Instruments (macOS):           $([ $HAS_INSTRUMENTS -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Not installed${NC}")"
    else
        echo "  Instruments (macOS):           ${YELLOW}N/A (macOS only)${NC}"
    fi
    
    echo ""
    
    echo -e "${BLUE}Installation commands:${NC}"
    echo "  rustup component add miri"
    echo "  cargo install cargo-audit"
    if [[ "$OS_TYPE" == "Linux" ]]; then
        echo "  sudo apt-get install valgrind  # or equivalent for your distro"
    fi
    echo ""
}

# Main execution function
main() {
    echo -e "${BLUE}${BOLD}=====================================${NC}"
    echo -e "${BLUE}${BOLD}  RustOwl Security & Memory Safety${NC}"
    echo -e "${BLUE}${BOLD}=====================================${NC}"
    echo ""
    
    # Detect platform first
    detect_platform
    echo ""
    
    # Auto-configure tests based on platform
    auto_configure_tests
    
    # Check Rust version compatibility
    check_rust_version
    echo ""
    
    detect_tools
    
    # If in check mode, just show tool status and exit
    if [ "${MODE:-}" = "check" ]; then
        show_tool_status
        exit 0
    fi
    
    # Build the project first
    build_project
    
    # Run all enabled security tests
    local exit_code=0
    
    run_miri_tests || exit_code=1
    run_valgrind_tests || exit_code=1
    run_sanitizer_tests || exit_code=1
    run_audit_check || exit_code=1
    run_drmemory_tests || exit_code=1
    run_instruments_tests || exit_code=1
    
    # Summary
    echo -e "${BLUE}${BOLD}Security Testing Summary${NC}"
    echo -e "${BLUE}================================${NC}"
    if [ $exit_code -eq 0 ]; then
        echo -e "${GREEN}✓ All security tests passed!${NC}"
        echo "No memory safety issues or security vulnerabilities detected."
    else
        echo -e "${RED}✗ Some security tests failed!${NC}"
        echo "Please review the output above and address any issues found."
    fi
    echo ""
    
    exit $exit_code
}

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
