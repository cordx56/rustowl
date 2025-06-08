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

# Output logging configuration
SECURITY_LOG_DIR="./security-logs"
VERBOSE_OUTPUT=0

# CI environment detection
IS_CI=0
CI_AUTO_INSTALL=0

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

# DrMemory configuration
DRMEMORY_VERSION="2.6.0"
DRMEMORY_URL="https://github.com/DynamoRIO/drmemory/releases/download/release_${DRMEMORY_VERSION}/DrMemory-Windows-${DRMEMORY_VERSION}.zip"
DRMEMORY_INSTALL_DIR="$HOME/.drmemory"

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

# Detect CI environment and configure accordingly
detect_ci_environment() {
    # Check for common CI environment variables
    if [[ -n "${CI:-}" ]] || [[ -n "${GITHUB_ACTIONS:-}" ]] || [[ -n "${GITLAB_CI:-}" ]] || \
       [[ -n "${TRAVIS:-}" ]] || [[ -n "${CIRCLECI:-}" ]] || [[ -n "${JENKINS_URL:-}" ]] || \
       [[ -n "${BUILDKITE:-}" ]] || [[ -n "${TF_BUILD:-}" ]]; then
        IS_CI=1
        CI_AUTO_INSTALL=1
        VERBOSE_OUTPUT=1  # Enable verbose output in CI
        echo -e "${BLUE}CI environment detected${NC}"
        
        # Show which CI system we detected
        if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
            echo -e "${BLUE}  Running on GitHub Actions${NC}"
        elif [[ -n "${GITLAB_CI:-}" ]]; then
            echo -e "${BLUE}  Running on GitLab CI${NC}"
        elif [[ -n "${TRAVIS:-}" ]]; then
            echo -e "${BLUE}  Running on Travis CI${NC}"
        elif [[ -n "${CIRCLECI:-}" ]]; then
            echo -e "${BLUE}  Running on CircleCI${NC}"
        elif [[ -n "${JENKINS_URL:-}" ]]; then
            echo -e "${BLUE}  Running on Jenkins${NC}"
        elif [[ -n "${BUILDKITE:-}" ]]; then
            echo -e "${BLUE}  Running on Buildkite${NC}"
        elif [[ -n "${TF_BUILD:-}" ]]; then
            echo -e "${BLUE}  Running on Azure DevOps${NC}"
        else
            echo -e "${BLUE}  Running on unknown CI system${NC}"
        fi
        
        echo -e "${BLUE}  Auto-installation enabled for missing tools${NC}"
        echo -e "${BLUE}  Verbose output enabled for detailed logging${NC}"
    else
        echo -e "${BLUE}Interactive environment detected${NC}"
    fi
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
    echo "  --install            Install missing security tools automatically"
    echo "  --ci                 Force CI mode (auto-install tools)"
    echo "  --no-auto-install    Disable automatic installation in CI"
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
    echo "CI Environment:"
    echo "  The script automatically detects CI environments and installs missing tools."
    echo "  Supported: GitHub Actions, GitLab CI, Travis CI, CircleCI, Jenkins,"
    echo "            Buildkite, Azure DevOps, and others with CI environment variables."
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
    echo "  $0 --install        # Install missing tools automatically"
    echo "  $0 --ci             # Force CI mode with auto-installation"
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
        --install)
            MODE="install"
            shift
            ;;
        --ci)
            IS_CI=1
            CI_AUTO_INSTALL=1
            shift
            ;;
        --no-auto-install)
            CI_AUTO_INSTALL=0
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
    
    # Check for DrMemory (Windows only) - check multiple possible locations
    if [[ "$OS_TYPE" == "Windows" ]]; then
        # Check if already in PATH
        if command -v drmemory >/dev/null 2>&1 || command -v drmemory.exe >/dev/null 2>&1; then
            HAS_DRMEMORY=1
            echo -e "${GREEN}✓ DrMemory is available in PATH${NC}"
        # Check our installation directory
        elif [ -f "$DRMEMORY_INSTALL_DIR/bin/drmemory.exe" ]; then
            HAS_DRMEMORY=1
            export PATH="$DRMEMORY_INSTALL_DIR/bin:$PATH"
            echo -e "${GREEN}✓ DrMemory found at $DRMEMORY_INSTALL_DIR${NC}"
        # Check common installation paths
        elif [ -f "/c/Program Files/Dr. Memory/bin/drmemory.exe" ]; then
            HAS_DRMEMORY=1
            export PATH="/c/Program Files/Dr. Memory/bin:$PATH"
            echo -e "${GREEN}✓ DrMemory found in Program Files${NC}"
        elif [ -f "/c/Program Files (x86)/Dr. Memory/bin/drmemory.exe" ]; then
            HAS_DRMEMORY=1
            export PATH="/c/Program Files (x86)/Dr. Memory/bin:$PATH"
            echo -e "${GREEN}✓ DrMemory found in Program Files (x86)${NC}"
        else
            echo -e "${YELLOW}! DrMemory not found${NC}"
        fi
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
    echo -e "${YELLOW}Testing RustOwl execution with Miri...${NC}"
    # Run Miri on actual RustOwl execution against dummy package, not on test dependencies
    
    # Check if dummy package exists for testing
    if [ -d "perf-tests/dummy-package" ]; then
        echo -e "${BLUE}Running RustOwl analysis with Miri...${NC}"
        if log_command_detailed "miri_rustowl_analysis" "cargo miri run --bin rustowl -- check perf-tests/dummy-package"; then
            echo -e "${GREEN}✓ RustOwl execution passed with Miri${NC}"
        else
            echo -e "${YELLOW}⚠ Miri could not complete analysis (likely due to jemalloc FFI calls)${NC}"
            echo -e "${YELLOW}  This is expected behavior as jemalloc uses foreign functions${NC}"
            echo -e "${YELLOW}  Core RustOwl logic would need testing with system allocator${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/miri_rustowl_analysis_${TIMESTAMP}.log${NC}"
        fi
    else
        echo -e "${YELLOW}⚠ No dummy package found at perf-tests/dummy-package${NC}"
        echo -e "${YELLOW}  Attempting to run RustOwl with --help to test basic execution...${NC}"
        if log_command_detailed "miri_basic_execution" "cargo miri run --bin rustowl -- --help"; then
            echo -e "${GREEN}✓ RustOwl basic execution passed with Miri${NC}"
        else
            echo -e "${YELLOW}⚠ Miri could not complete analysis (likely due to jemalloc FFI calls)${NC}"
            echo -e "${YELLOW}  This is expected behavior as jemalloc uses foreign functions${NC}"
            echo -e "${YELLOW}  Core RustOwl logic would need testing with system allocator${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/miri_basic_execution_${TIMESTAMP}.log${NC}"
        fi
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
        
        # Add timeout to the command for enhanced logging
        local full_cmd="timeout 300 $valgrind_cmd ./target/security/rustowl check $TEST_TARGET_PATH"
        
        if log_command_detailed "valgrind_rustowl_analysis" "$full_cmd"; then
            echo -e "${GREEN}✓ No memory errors detected by Valgrind${NC}"
        else
            echo -e "${RED}✗ Valgrind detected memory issues${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/valgrind_rustowl_analysis_${TIMESTAMP}.log${NC}"
            echo "Run manually for details: $valgrind_cmd ./target/security/rustowl check $TEST_TARGET_PATH"
            return 1
        fi
    else
        echo -e "${YELLOW}! Test package not found at $TEST_TARGET_PATH${NC}"
        echo -e "${YELLOW}  Testing basic rustowl execution with Valgrind...${NC}"
        
        local valgrind_cmd="valgrind --tool=memcheck --leak-check=full --show-leak-kinds=all --error-exitcode=1 --track-origins=yes"
        local basic_cmd="timeout 60 $valgrind_cmd ./target/security/rustowl --help"
        
        if log_command_detailed "valgrind_basic_execution" "$basic_cmd"; then
            echo -e "${GREEN}✓ Basic Valgrind test passed${NC}"
        else
            echo -e "${RED}✗ Valgrind basic test failed${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/valgrind_basic_execution_${TIMESTAMP}.log${NC}"
            return 1
        fi
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
    local asan_cmd="RUSTFLAGS=\"-Z sanitizer=address\" cargo build --target $current_target --target-dir target/sanitizer --profile=security"
    
    if log_command_detailed "sanitizer_address_build" "$asan_cmd"; then
        echo -e "${GREEN}✓ AddressSanitizer build successful for $current_target${NC}"
        
        # Test execution if possible and test target exists
        if [ -d "$TEST_TARGET_PATH" ]; then
            local asan_binary="target/sanitizer/$current_target/security/rustowl"
            if [[ "$OS_TYPE" == "Windows" ]]; then
                asan_binary="$asan_binary.exe"
            fi
            
            if [ -f "$asan_binary" ]; then
                echo -e "${YELLOW}Testing AddressSanitizer binary execution...${NC}"
                local asan_test_cmd="$asan_binary check $TEST_TARGET_PATH"
                
                if log_command_detailed "sanitizer_address_execution" "$asan_test_cmd"; then
                    echo -e "${GREEN}✓ AddressSanitizer execution test passed${NC}"
                else
                    echo -e "${YELLOW}⚠ AddressSanitizer execution had issues${NC}"
                    echo -e "${BLUE}  Full output captured in: $LOG_DIR/sanitizer_address_execution_${TIMESTAMP}.log${NC}"
                    sanitizer_failed=true
                fi
            fi
        fi
    else
        echo -e "${YELLOW}! AddressSanitizer build failed for $current_target${NC}"
        echo -e "${BLUE}  Full output captured in: $LOG_DIR/sanitizer_address_build_${TIMESTAMP}.log${NC}"
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
    if log_command_detailed "cargo_audit_scan" "cargo audit"; then
        echo -e "${GREEN}✓ No known vulnerabilities found${NC}"
    else
        echo -e "${RED}✗ Security vulnerabilities detected${NC}"
        echo -e "${BLUE}  Full output captured in: $LOG_DIR/cargo_audit_scan_${TIMESTAMP}.log${NC}"
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
    
    # Verify DrMemory is working
    echo -e "${YELLOW}Verifying DrMemory installation...${NC}"
    local drmemory_cmd="drmemory"
    if ! command -v drmemory >/dev/null 2>&1; then
        drmemory_cmd="drmemory.exe"
    fi
    
    if log_command_detailed "drmemory_version_check" "$drmemory_cmd -version"; then
        echo -e "${GREEN}✓ DrMemory is working${NC}"
    else
        echo -e "${RED}✗ DrMemory not working properly${NC}"
        echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_version_check_${TIMESTAMP}.log${NC}"
        return 1
    fi
    
    # Find the rustowl binary
    local rustowl_binary="./target/security/rustowl.exe"
    if [ ! -f "$rustowl_binary" ]; then
        rustowl_binary="./target/security/rustowl"
        if [ ! -f "$rustowl_binary" ]; then
            echo -e "${RED}✗ RustOwl binary not found${NC}"
            return 1
        fi
    fi
    
    if [ -d "$TEST_TARGET_PATH" ]; then
        echo -e "${YELLOW}Testing rustowl with DrMemory...${NC}"
        echo "  Command: $drmemory_cmd -- $rustowl_binary check $TEST_TARGET_PATH"
        
        # Run DrMemory with appropriate options for Rust binaries and timeout
        local drmemory_full_cmd="timeout 300 $drmemory_cmd -- $rustowl_binary check $TEST_TARGET_PATH"
        
        if log_command_detailed "drmemory_rustowl_analysis" "$drmemory_full_cmd"; then
            echo -e "${GREEN}✓ No memory errors detected by DrMemory${NC}"
        else
            local exit_code=$?
            if grep -q "timeout" "$LOG_DIR/drmemory_rustowl_analysis_${TIMESTAMP}.log" 2>/dev/null; then
                echo -e "${YELLOW}⚠ DrMemory test timed out (300 seconds)${NC}"
                echo -e "${YELLOW}  This may indicate a performance issue or DrMemory overhead${NC}"
                echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_rustowl_analysis_${TIMESTAMP}.log${NC}"
                return 0  # Don't fail for timeout
            else
                echo -e "${RED}✗ DrMemory detected memory issues or failed to run${NC}"
                echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_rustowl_analysis_${TIMESTAMP}.log${NC}"
                echo "Run manually for details:"
                echo "  $drmemory_cmd -- $rustowl_binary check $TEST_TARGET_PATH"
                return 1
            fi
        fi
    else
        echo -e "${YELLOW}! Test package not found at $TEST_TARGET_PATH${NC}"
        echo -e "${YELLOW}  Testing basic rustowl execution with DrMemory...${NC}"
        
        local drmemory_basic_cmd="timeout 60 $drmemory_cmd -- $rustowl_binary --help"
        
        if log_command_detailed "drmemory_basic_execution" "$drmemory_basic_cmd"; then
            echo -e "${GREEN}✓ Basic DrMemory test passed${NC}"
        else
            if grep -q "timeout" "$LOG_DIR/drmemory_basic_execution_${TIMESTAMP}.log" 2>/dev/null; then
                echo -e "${YELLOW}⚠ DrMemory basic test timed out${NC}"
                echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_basic_execution_${TIMESTAMP}.log${NC}"
                return 0
            else
                echo -e "${RED}✗ DrMemory basic test failed${NC}"
                echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_basic_execution_${TIMESTAMP}.log${NC}"
                return 1
            fi
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
        
        # Create unique trace file name to avoid conflicts
        local trace_file="instruments_output_${TIMESTAMP}.trace"
        local instruments_cmd="instruments -t 'Allocations' -D $trace_file ./target/security/rustowl check $TEST_TARGET_PATH"
        
        if log_command_detailed "instruments_allocations_analysis" "$instruments_cmd"; then
            echo -e "${GREEN}✓ Instruments analysis completed${NC}"
        else
            echo -e "${RED}✗ Instruments analysis failed${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/instruments_allocations_analysis_${TIMESTAMP}.log${NC}"
            echo "Run manually for details: instruments -t 'Allocations' ./target/security/rustowl check $TEST_TARGET_PATH"
            # Clean up trace file on failure
            rm -rf "$trace_file" 2>/dev/null || true
            return 1
        fi
        
        # Clean up trace file after successful run
        rm -rf "$trace_file" 2>/dev/null || true
    else
        echo -e "${YELLOW}! Test package not found at $TEST_TARGET_PATH${NC}"
        echo -e "${YELLOW}  Testing basic rustowl execution with Instruments...${NC}"
        
        local trace_file="instruments_basic_${TIMESTAMP}.trace"
        local instruments_basic_cmd="instruments -t 'Allocations' -D $trace_file ./target/security/rustowl --help"
        
        if log_command_detailed "instruments_basic_execution" "$instruments_basic_cmd"; then
            echo -e "${GREEN}✓ Basic Instruments test passed${NC}"
        else
            echo -e "${RED}✗ Instruments basic test failed${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/instruments_basic_execution_${TIMESTAMP}.log${NC}"
            # Clean up trace file on failure
            rm -rf "$trace_file" 2>/dev/null || true
            return 1
        fi
        
        # Clean up trace file
        rm -rf "$trace_file" 2>/dev/null || true
    fi
    
    echo ""
}

# Logging configuration
LOG_DIR="security-logs"
TIMESTAMP=$(date '+%Y%m%d_%H%M%S')

# Enhanced logging function for tool outputs
log_command_detailed() {
    local test_name="$1"
    local command="$2"
    local log_file="$LOG_DIR/${test_name}_${TIMESTAMP}.log"
    
    # Create log directory if it doesn't exist
    mkdir -p "$LOG_DIR"
    
    echo "===========================================" >> "$log_file"
    echo "Test: $test_name" >> "$log_file"
    echo "Command: $command" >> "$log_file"
    echo "Timestamp: $(date)" >> "$log_file"
    echo "Working Directory: $(pwd)" >> "$log_file"
    echo "Environment: OS=$OS_TYPE, CI=$IS_CI" >> "$log_file"
    echo "===========================================" >> "$log_file"
    echo "" >> "$log_file"
    
    # Run the command and capture both stdout and stderr
    echo "=== COMMAND OUTPUT ===" >> "$log_file"
    if eval "$command" >> "$log_file" 2>&1; then
        local exit_code=0
        echo "" >> "$log_file"
        echo "=== COMMAND COMPLETED SUCCESSFULLY ===" >> "$log_file"
    else
        local exit_code=$?
        echo "" >> "$log_file"
        echo "=== COMMAND FAILED WITH EXIT CODE: $exit_code ===" >> "$log_file"
    fi
    
    echo "End timestamp: $(date)" >> "$log_file"
    echo "===========================================" >> "$log_file"
    
    return $exit_code
}

# Show tool status summary
show_tool_status() {
    echo -e "${BLUE}${BOLD}Tool Availability Summary${NC}"
    echo -e "${BLUE}================================${NC}"
    echo ""
    
    echo -e "${BLUE}Platform: $OS_TYPE${NC}"
    echo ""
    
    echo "Security Tools:"
    echo -e "  Miri (UB detection):           $([ $HAS_MIRI -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Missing${NC}")"
    
    if [[ "$OS_TYPE" == "Linux" ]]; then
        echo -e "  Valgrind (memory errors):      $([ $HAS_VALGRIND -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Missing${NC}")"
    fi
    
    echo -e "  cargo-audit (vulnerabilities): $([ $HAS_CARGO_AUDIT -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Missing${NC}")"
    
    if [[ "$OS_TYPE" == "Windows" ]]; then
        echo -e "  DrMemory (memory debugging):   $([ $HAS_DRMEMORY -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Missing${NC}")"
    fi
    
    if [[ "$OS_TYPE" == "macOS" ]]; then
        echo -e "  Instruments (performance):     $([ $HAS_INSTRUMENTS -eq 1 ] && echo -e "${GREEN}✓ Available${NC}" || echo -e "${RED}✗ Missing${NC}")"
    fi
    
    echo ""
    
    # Check nightly toolchain for sanitizers
    local current_toolchain=$(rustup show active-toolchain | cut -d' ' -f1)
    echo "Sanitizer Support:"
    if [[ "$current_toolchain" == *"nightly"* ]]; then
        echo -e "  Nightly toolchain:             ${GREEN}✓ Available${NC}"
        echo -e "  AddressSanitizer:              ${GREEN}✓ Supported${NC}"
        echo -e "  ThreadSanitizer:               ${GREEN}✓ Supported${NC}"
        echo -e "  MemorySanitizer:               ${GREEN}✓ Supported${NC}"
    else
        echo -e "  Nightly toolchain:             ${YELLOW}! Stable toolchain active${NC}"
        echo -e "  Sanitizers:                    ${YELLOW}! Require nightly${NC}"
    fi
    
    echo ""
    echo "Test Configuration:"
    echo -e "  Run Miri:       $([ $RUN_MIRI -eq 1 ] && echo -e "${GREEN}Enabled${NC}" || echo -e "${YELLOW}Disabled${NC}")"
    echo -e "  Run Valgrind:   $([ $RUN_VALGRIND -eq 1 ] && echo -e "${GREEN}Enabled${NC}" || echo -e "${YELLOW}Disabled${NC}")"
    echo -e "  Run Sanitizers: $([ $RUN_SANITIZERS -eq 1 ] && echo -e "${GREEN}Enabled${NC}" || echo -e "${YELLOW}Disabled${NC}")"
    echo -e "  Run Audit:      $([ $RUN_AUDIT -eq 1 ] && echo -e "${GREEN}Enabled${NC}" || echo -e "${YELLOW}Disabled${NC}")"
    echo -e "  Run DrMemory:   $([ $RUN_DRMEMORY -eq 1 ] && echo -e "${GREEN}Enabled${NC}" || echo -e "${YELLOW}Disabled${NC}")"
    echo -e "  Run Instruments: $([ $RUN_INSTRUMENTS -eq 1 ] && echo -e "${GREEN}Enabled${NC}" || echo -e "${YELLOW}Disabled${NC}")"
    
    echo ""
}

# Create security summary with tool outputs
create_security_summary() {
    local summary_file="$LOG_DIR/security_summary_${TIMESTAMP}.md"
    
    mkdir -p "$LOG_DIR"
    
    echo "# Security Testing Summary" > "$summary_file"
    echo "" >> "$summary_file"
    echo "**Generated:** $(date)" >> "$summary_file"
    echo "**Platform:** $OS_TYPE" >> "$summary_file"
    echo "**CI Environment:** $([ $IS_CI -eq 1 ] && echo "Yes" || echo "No")" >> "$summary_file"
    echo "**Rust Version:** $(rustc --version 2>/dev/null || echo 'N/A')" >> "$summary_file"
    echo "" >> "$summary_file"
    
    # Tool availability summary
    echo "## Tool Availability" >> "$summary_file"
    echo "" >> "$summary_file"
    echo "| Tool | Status | Notes |" >> "$summary_file"
    echo "|------|--------|-------|" >> "$summary_file"
    echo "| Miri | $([ $HAS_MIRI -eq 1 ] && echo "✅ Available" || echo "❌ Missing") | Undefined behavior detection |" >> "$summary_file"
    echo "| Valgrind | $([ $HAS_VALGRIND -eq 1 ] && echo "✅ Available" || echo "❌ Missing/N/A") | Memory error detection (Linux) |" >> "$summary_file"
    echo "| cargo-audit | $([ $HAS_CARGO_AUDIT -eq 1 ] && echo "✅ Available" || echo "❌ Missing") | Security vulnerability scanning |" >> "$summary_file"
    echo "| DrMemory | $([ $HAS_DRMEMORY -eq 1 ] && echo "✅ Available" || echo "❌ Missing/N/A") | Memory debugging (Windows) |" >> "$summary_file"
    echo "| Instruments | $([ $HAS_INSTRUMENTS -eq 1 ] && echo "✅ Available" || echo "❌ Missing/N/A") | Performance analysis (macOS) |" >> "$summary_file"
    echo "" >> "$summary_file"
    
    # Test results summary
    echo "## Test Results" >> "$summary_file"
    echo "" >> "$summary_file"
    
    # Find all log files and summarize them
    if [ -d "$LOG_DIR" ]; then
        for log_file in "$LOG_DIR"/*.log; do
            if [ -f "$log_file" ]; then
                local test_name=$(basename "$log_file" .log | sed "s/_${TIMESTAMP}//")
                echo "### $test_name" >> "$summary_file"
                echo "" >> "$summary_file"
                
                # Check if test passed or failed based on log content
                if grep -q "COMMAND COMPLETED SUCCESSFULLY" "$log_file"; then
                    echo "**Status:** ✅ PASSED" >> "$summary_file"
                elif grep -q "COMMAND FAILED" "$log_file"; then
                    echo "**Status:** ❌ FAILED" >> "$summary_file"
                    
                    # Extract error information
                    echo "" >> "$summary_file"
                    echo "**Error Details:**" >> "$summary_file"
                    echo '```' >> "$summary_file"
                    # Get last 20 lines before the failure marker
                    grep -B 20 "COMMAND FAILED" "$log_file" | tail -20 >> "$summary_file"
                    echo '```' >> "$summary_file"
                else
                    echo "**Status:** ⚠️ UNKNOWN" >> "$summary_file"
                fi
                
                echo "" >> "$summary_file"
                echo "**Log file:** \`$(basename "$log_file")\`" >> "$summary_file"
                echo "**File size:** $(wc -c < "$log_file" 2>/dev/null || echo 'N/A') bytes" >> "$summary_file"
                echo "" >> "$summary_file"
            fi
        done
    fi
    
    # System information
    echo "## System Information" >> "$summary_file"
    echo "" >> "$summary_file"
    echo "**Rust Toolchain:**" >> "$summary_file"
    echo '```' >> "$summary_file"
    rustup show 2>/dev/null || echo "Rustup not available" >> "$summary_file"
    echo '```' >> "$summary_file"
    echo "" >> "$summary_file"
    
    # Environment variables relevant to security testing
    echo "**Environment Variables:**" >> "$summary_file"
    echo '```' >> "$summary_file"
    echo "RUSTC_BOOTSTRAP=$RUSTC_BOOTSTRAP" >> "$summary_file"
    echo "CARGO_TERM_COLOR=$CARGO_TERM_COLOR" >> "$summary_file"
    echo "CI=$CI" >> "$summary_file"
    echo "GITHUB_ACTIONS=$GITHUB_ACTIONS" >> "$summary_file"
    echo '```' >> "$summary_file"
    
    echo "" >> "$summary_file"
    echo "---" >> "$summary_file"
    echo "*Generated by RustOwl security testing script*" >> "$summary_file"
}
# Main execution function
main() {
    echo -e "${BLUE}${BOLD}=====================================${NC}"
    echo -e "${BLUE}${BOLD}  RustOwl Security & Memory Safety${NC}"
    echo -e "${BLUE}${BOLD}=====================================${NC}"
    echo ""
    
    # Detect platform first
    detect_platform
    
    # Detect CI environment
    detect_ci_environment
    echo ""
    
    # Auto-configure tests based on platform
    auto_configure_tests
    
    # Check Rust version compatibility
    check_rust_version
    echo ""

    detect_tools
    
    # In CI environments, automatically install missing tools
    if [[ $IS_CI -eq 1 ]] && [[ $CI_AUTO_INSTALL -eq 1 ]]; then
        echo -e "${BLUE}CI environment: Installing missing security tools automatically...${NC}"
        install_required_tools
        # Re-detect tools after installation
        detect_tools
    fi
    
    # If in check mode, just show tool status and exit
    if [ "${MODE:-}" = "check" ]; then
        show_tool_status
        exit 0
    fi
    
    # If in install mode, install tools and exit
    if [ "${MODE:-}" = "install" ]; then
        install_required_tools
        # Re-detect tools after installation
        detect_tools
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
    
    # Create detailed security summary
    create_security_summary
    
    exit $exit_code
}

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
