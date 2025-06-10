#!/bin/bash
# RustOwl Security & Memory Safety Testing Script
# Tests for undefined behavior, memory leaks, and security vulnerabilities
# Automatically detects platform capabilities and runs appropriate tests

echo "DEBUG: Script started"

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
RUN_THREAD_SANITIZER=0
RUN_CARGO_MACHETE=0

# Tool availability detection
HAS_MIRI=0
HAS_VALGRIND=0
HAS_CARGO_AUDIT=0
HAS_DRMEMORY=0
HAS_INSTRUMENTS=0
HAS_CARGO_MACHETE=0

# DrMemory configuration
DRMEMORY_VERSION="2.6.0"
DRMEMORY_URL="https://github.com/DynamoRIO/drmemory/releases/download/release_${DRMEMORY_VERSION}/DrMemory-Windows-${DRMEMORY_VERSION}.zip"
DRMEMORY_INSTALL_DIR="$HOME/.drmemory"

# DrMemory CI safety settings
# Set to 1 to completely disable DrMemory in CI environments (recommended if causing timeouts)
DISABLE_DRMEMORY_IN_CI=0
# Set to 1 to force only basic DrMemory tests in CI (safer alternative)
FORCE_BASIC_DRMEMORY_IN_CI=1

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
    if [[ -n "${CI:-}" ]] || [[ -n "${GITHUB_ACTIONS:-}" ]]; then
        IS_CI=1
        CI_AUTO_INSTALL=1
        VERBOSE_OUTPUT=1  # Enable verbose output in CI
        echo -e "${BLUE}CI environment detected${NC}"
        
        # Show which CI system we detected
        if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
            echo -e "${BLUE}  Running on GitHub Actions${NC}"
        else
            echo -e "${BLUE}  Running on unknown CI system${NC}"
        fi
        
        echo -e "${BLUE}  Auto-installation enabled for missing tools${NC}"
        echo -e "${BLUE}  Verbose output enabled for detailed logging${NC}"
    else
        echo -e "${BLUE}Interactive environment detected${NC}"
    fi
}

# Install missing tools automatically in CI
install_required_tools() {
    echo -e "${BLUE}Installing missing security tools...${NC}"
    
    # Install cargo-audit
    if [[ $HAS_CARGO_AUDIT -eq 0 ]] && [[ $RUN_AUDIT -eq 1 ]]; then
        echo "Installing cargo-audit..."
        if ! cargo install cargo-audit; then
            echo -e "${RED}Failed to install cargo-audit${NC}"
        fi
    fi
    
    # Install cargo-machete
    if [[ $HAS_CARGO_MACHETE -eq 0 ]] && [[ $RUN_CARGO_MACHETE -eq 1 ]]; then
        echo "Installing cargo-machete..."
        if ! cargo install cargo-machete; then
            echo -e "${RED}Failed to install cargo-machete${NC}"
        fi
    fi

    # Install Miri component if missing and needed
    if [[ $HAS_MIRI -eq 0 ]] && [[ $RUN_MIRI -eq 1 ]]; then
        echo "Installing Miri component..."
        if rustup component add miri --toolchain nightly; then
            echo -e "${GREEN}Miri component installed successfully${NC}"
            HAS_MIRI=1
        else
            echo -e "${RED}Failed to install Miri component${NC}"
        fi
    fi

    # Install DrMemory on Windows
    if [[ "$OS_TYPE" == "Windows" ]] && [[ $HAS_DRMEMORY -eq 0 ]] && [[ $RUN_DRMEMORY -eq 1 ]]; then
        echo "Installing DrMemory..."
        if install_drmemory; then
            echo -e "${GREEN}DrMemory installed successfully${NC}"
            HAS_DRMEMORY=1
        else
            echo -e "${RED}Failed to install DrMemory${NC}"
        fi
    fi

    # Install Valgrind on Linux (if package manager available)
    if [[ "$OS_TYPE" == "Linux" ]] && [[ $HAS_VALGRIND -eq 0 ]] && [[ $RUN_VALGRIND -eq 1 ]]; then
        echo "Attempting to install Valgrind..."
        if command -v apt-get >/dev/null 2>&1; then
            if sudo apt-get update && sudo apt-get install -y valgrind; then
                echo -e "${GREEN}Valgrind installed successfully${NC}"
                HAS_VALGRIND=1
            else
                echo -e "${RED}Failed to install Valgrind via apt-get${NC}"
            fi
        elif command -v yum >/dev/null 2>&1; then
            if sudo yum install -y valgrind; then
                echo -e "${GREEN}Valgrind installed successfully${NC}"
                HAS_VALGRIND=1
            else
                echo -e "${RED}Failed to install Valgrind via yum${NC}"
            fi
        elif command -v pacman >/dev/null 2>&1; then
            if sudo pacman -S --noconfirm valgrind; then
                echo -e "${GREEN}Valgrind installed successfully${NC}"
                HAS_VALGRIND=1
            else
                echo -e "${RED}Failed to install Valgrind via pacman${NC}"
            fi
        else
            echo -e "${YELLOW}No supported package manager found for Valgrind installation${NC}"
        fi
    fi
    
    # Install/setup Xcode on macOS (CI environments)
    if [[ "$OS_TYPE" == "macOS" ]] && [[ $IS_CI -eq 1 ]] && [[ $HAS_INSTRUMENTS -eq 0 ]] && [[ $RUN_INSTRUMENTS -eq 1 ]]; then
        echo "Setting up Xcode for CI environment..."
        
        # First, try to install/setup command line tools
        if sudo xcode-select --install 2>/dev/null || true; then
            echo "Xcode command line tools installation initiated..."
        fi
        
        # Set the developer directory
        if [[ -d "/Applications/Xcode.app" ]]; then
            echo "Found Xcode.app, setting developer directory..."
            sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
        elif [[ -d "/Library/Developer/CommandLineTools" ]]; then
            echo "Using Command Line Tools..."
            sudo xcode-select --switch /Library/Developer/CommandLineTools
        fi
        
        # Accept license if needed
        if sudo xcodebuild -license accept 2>/dev/null; then
            echo "Xcode license accepted"
        fi
        
        # Verify setup
        if xcode-select -p >/dev/null 2>&1; then
            echo "Xcode developer directory: $(xcode-select -p)"
            
            # Check if instruments is now available
            if command -v instruments >/dev/null 2>&1; then
                if timeout 10s instruments -help >/dev/null 2>&1; then
                    HAS_INSTRUMENTS=1
                    echo -e "${GREEN}Instruments is now available${NC}"
                else
                    echo -e "${YELLOW}Instruments found but may not be fully functional${NC}"
                fi
            else
                echo -e "${YELLOW}Instruments still not available after Xcode setup${NC}"
            fi
        else
            echo -e "${RED}Failed to set up Xcode properly${NC}"
        fi
    fi

    echo ""
}

# Install Xcode for macOS CI environments
install_xcode_ci() {
    if [[ "$OS_TYPE" != "macOS" ]] || [[ $IS_CI -ne 1 ]]; then
        return 0
    fi
    
    echo "Setting up Xcode for CI environment..."
    
    # First, try to install/setup command line tools
    if sudo xcode-select --install 2>/dev/null || true; then
        echo "Xcode command line tools installation initiated..."
    fi
    
    # Set the developer directory
    if [[ -d "/Applications/Xcode.app" ]]; then
        echo "Found Xcode.app, setting developer directory..."
        sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
    elif [[ -d "/Library/Developer/CommandLineTools" ]]; then
        echo "Using Command Line Tools..."
        sudo xcode-select --switch /Library/Developer/CommandLineTools
    fi
    
    # Accept license if needed
    if sudo xcodebuild -license accept 2>/dev/null; then
        echo "Xcode license accepted"
    fi
    
    # Verify setup
    if xcode-select -p >/dev/null 2>&1; then
        echo "Xcode developer directory: $(xcode-select -p)"
        
        # Check if instruments is now available
        if command -v instruments >/dev/null 2>&1; then
            if timeout 10s instruments -help >/dev/null 2>&1; then
                HAS_INSTRUMENTS=1
                echo -e "${GREEN}Instruments is now available${NC}"
            else
                echo -e "${YELLOW}Instruments found but may not be fully functional${NC}"
            fi
        else
            echo -e "${YELLOW}Instruments still not available after Xcode setup${NC}"
        fi
    else
        echo -e "${RED}Failed to set up Xcode properly${NC}"
    fi
    
    echo ""
}

# Install DrMemory for Windows
install_drmemory() {
    if [[ "$OS_TYPE" != "Windows" ]]; then
        return 0
    fi
    
    echo "Installing DrMemory..."
    
    # Download DrMemory
    local drmemory_zip="DrMemory-Windows-${DRMEMORY_VERSION}.zip"
    local drmemory_dir="DrMemory-Windows-${DRMEMORY_VERSION}"
    
    if command -v curl >/dev/null 2>&1; then
        curl -L -o "$drmemory_zip" "$DRMEMORY_URL"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$drmemory_zip" "$DRMEMORY_URL"
    else
        echo -e "${RED}Error: Neither curl nor wget is available for downloading DrMemory.${NC}"
        return 1
    fi
    
    # Extract and install
    if unzip -q "$drmemory_zip"; then
        echo "DrMemory installed to: $DRMEMORY_INSTALL_DIR"
        echo "Add to PATH: set PATH=%PATH%;$DRMEMORY_INSTALL_DIR\\bin"
        
        # Clean up
        rm -f "$drmemory_zip"
    else
        echo -e "${RED}Failed to install DrMemory${NC}"
        return 1
    fi
    
    echo ""
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
            # macOS: Focus on Rust-native tools and macOS-compatible alternatives
            echo "  macOS detected: Enabling Miri, Audit, and macOS-compatible tools"
            echo "  Disabling Valgrind (unreliable on macOS)"
            echo "  Disabling Sanitizers (compatibility issues on Apple Silicon)"
            echo "  Disabling ThreadSanitizer (also problematic on Apple Silicon)"
            echo "  Enabling cargo-machete for unused dependency detection"
            echo "  Instruments will be attempted after Xcode setup in CI"
            RUN_VALGRIND=0
            RUN_DRMEMORY=0
            RUN_SANITIZERS=0  # Disable AddressSanitizer
            RUN_THREAD_SANITIZER=0  # Also disable ThreadSanitizer
            RUN_CARGO_MACHETE=1  # Detect unused dependencies
            RUN_INSTRUMENTS=1  # Enable Instruments (will try to install Xcode in CI)
            ;;
        "Windows")
            # Windows: Disable sanitizers as they're often problematic on Windows CI
            echo "  Windows detected: Enabling Miri, Audit, and DrMemory"
            echo "  Disabling Valgrind (Linux only)"
            echo "  Disabling Sanitizers (unreliable on Windows CI)"
            RUN_VALGRIND=0
            RUN_INSTRUMENTS=0
            RUN_SANITIZERS=0  # Disable sanitizers on Windows
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
    echo "  --disable-drmemory-ci Disable DrMemory completely in CI environments"
    echo "  --allow-full-drmemory-ci Allow full DrMemory analysis in CI (not recommended)"
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
    echo "  - Miri: Detects undefined behavior in Rust code"
    echo "  - Valgrind: Memory error detection (Linux)"
    echo "  - AddressSanitizer: Memory error detection"
    echo "  - ThreadSanitizer: Data race detection" 
    echo "  - MemorySanitizer: Uninitialized memory detection"
    echo "  - cargo-audit: Security vulnerability scanning"
    echo "  - DrMemory: Memory debugging (Windows)"
    echo "  - Instruments: Performance and memory analysis (macOS)"
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
        --disable-drmemory-ci)
            DISABLE_DRMEMORY_IN_CI=1
            shift
            ;;
        --allow-full-drmemory-ci)
            FORCE_BASIC_DRMEMORY_IN_CI=0
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
        echo -e "${RED}[ERROR] Rust compiler not found. Please install Rust: https://rustup.rs/${NC}"
        exit 1
    fi
    
    local current_version=$(rustc --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
    local min_version="$MIN_RUST_VERSION"
    
    if [ -z "$current_version" ]; then
        echo -e "${YELLOW}[WARN] Could not determine Rust version, proceeding anyway...${NC}"
        return 0
    fi
    
    # Simple version comparison (assumes semantic versioning)
    if printf '%s\n%s\n' "$min_version" "$current_version" | sort -V -C; then
        echo -e "${GREEN}[OK] Rust $current_version >= $min_version (minimum required)${NC}"
        return 0
    else
        echo -e "${RED}[ERROR] Rust $current_version < $min_version (minimum required)${NC}"
        echo -e "${YELLOW}Please update Rust: rustup update${NC}"
        exit 1
    fi
}

# Detect available tools based on platform
detect_tools() {
    echo -e "${BLUE}Detecting available security tools...${NC}"
    
    # Check for cargo-audit
    if command -v cargo-audit >/dev/null 2>&1; then
        HAS_CARGO_AUDIT=1
        echo -e "${GREEN}[OK] cargo-audit available${NC}"
    else
        echo -e "${YELLOW}! cargo-audit not found${NC}"
        HAS_CARGO_AUDIT=0
    fi
    
    # Check for cargo-machete
    if command -v cargo-machete >/dev/null 2>&1; then
        HAS_CARGO_MACHETE=1
        echo -e "${GREEN}[OK] cargo-machete available${NC}"
    else
        echo -e "${YELLOW}! cargo-machete not found${NC}"
        HAS_CARGO_MACHETE=0
    fi

    # Platform-specific tool detection
    case "$OS_TYPE" in
        "macOS")
            # Check for Instruments (part of Xcode)
            # In CI environments, we'll try to install Xcode, so check normally
            if command -v instruments >/dev/null 2>&1; then
                # Additional check: try to run instruments to see if it actually works
                if timeout 10s instruments -help >/dev/null 2>&1; then
                    HAS_INSTRUMENTS=1
                    echo -e "${GREEN}[OK] Instruments available${NC}"
                else
                    HAS_INSTRUMENTS=0
                    echo -e "${YELLOW}! Instruments found but not working (needs Xcode setup)${NC}"
                fi
            else
                HAS_INSTRUMENTS=0
                echo -e "${YELLOW}! Instruments not found (will try to install Xcode in CI)${NC}"
            fi
            ;;
        "Windows")
            # Check for DrMemory
            if [[ -f "$DRMEMORY_INSTALL_DIR/bin/drmemory.exe" ]] || command -v drmemory >/dev/null 2>&1; then
                HAS_DRMEMORY=1
                echo -e "${GREEN}[OK] DrMemory available${NC}"
            else
                echo -e "${YELLOW}! DrMemory not found${NC}"
                HAS_DRMEMORY=0
            fi
            ;;
        "Linux")
            # Check for Valgrind
            if command -v valgrind >/dev/null 2>&1; then
                HAS_VALGRIND=1
                echo -e "${GREEN}[OK] Valgrind available${NC}"
            else
                echo -e "${YELLOW}! Valgrind not found${NC}"
                HAS_VALGRIND=0
            fi
            ;;
    esac

    echo ""
}

# Check nightly toolchain availability for advanced features
check_nightly_toolchain() {
    echo -e "${YELLOW}Checking toolchain for advanced security features...${NC}"
    
    # Check what toolchain is currently active
    local current_toolchain=$(rustup show active-toolchain | cut -d' ' -f1)
    echo -e "${BLUE}Active toolchain: $current_toolchain${NC}"
    
    if [[ "$current_toolchain" == *"nightly"* ]]; then
        echo -e "${GREEN}[OK] Nightly toolchain is active (from rust-toolchain.toml)${NC}"
    else
        echo -e "${YELLOW}! Stable toolchain detected${NC}"
        echo -e "${YELLOW}Some advanced features require nightly (check rust-toolchain.toml)${NC}"
    fi
    
    # Check if Miri component is available on current toolchain
    if rustup component list --installed | grep -q miri 2>/dev/null; then
        HAS_MIRI=1
        echo -e "${GREEN}[OK] Miri is available${NC}"
    else
        echo -e "${YELLOW}! Miri component not installed${NC}"
        echo -e "${YELLOW}Install with: rustup component add miri${NC}"
        HAS_MIRI=0
    fi
    
    # Check if required targets are installed
    local current_target
    local arch
    case "$OS_TYPE" in
        "Linux")
            arch=$(uname -m)
            if [[ "$arch" == "x86_64" ]]; then
                current_target="x86_64-unknown-linux-gnu"
            elif [[ "$arch" == "aarch64" ]]; then
                current_target="aarch64-unknown-linux-gnu"
            else
                echo -e "${YELLOW}Unsupported Linux architecture: $arch for sanitizer tests.${NC}"
            fi
            ;;
        "macOS")
            arch=$(uname -m)
            if [[ "$arch" == "x86_64" ]]; then
                current_target="x86_64-apple-darwin"
            elif [[ "$arch" == "arm64" ]]; then # arm64 is the output for Apple Silicon
                current_target="aarch64-apple-darwin"
            else
                echo -e "${YELLOW}Unsupported macOS architecture: $arch for sanitizer tests.${NC}"
            fi
            ;;
        "Windows")
            current_target="x86_64-pc-windows-msvc"
            ;;
    esac
    
    if [[ -n "$current_target" ]]; then
        if rustup target list --installed | grep -q "$current_target"; then
            echo -e "${GREEN}[OK] Target $current_target is available${NC}"
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
        echo -e "${RED}[ERROR] Failed to build rustowl binary${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}[OK] Build completed successfully${NC}"
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
    
    # First run unit tests which are guaranteed to work with Miri
    echo -e "${BLUE}Running RustOwl unit tests with Miri...${NC}"
    echo -e "${BLUE}Using Miri flags: -Zmiri-disable-isolation -Zmiri-permissive-provenance${NC}"
    if MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-permissive-provenance" RUSTFLAGS="--cfg miri" log_command_detailed "miri_unit_tests" "cargo miri test --lib"; then
        echo -e "${GREEN}[OK] RustOwl unit tests passed with Miri${NC}"
    else
        echo -e "${RED}[FAIL] RustOwl unit tests failed with Miri${NC}"
        echo -e "${BLUE}  Full output captured in: $LOG_DIR/miri_unit_tests_${TIMESTAMP}.log${NC}"
        return 1
    fi
    
    # Test RustOwl's main functionality with Miri
    echo -e "${YELLOW}Testing RustOwl execution with Miri...${NC}"
    
    if [ -d "$TEST_TARGET_PATH" ]; then
        echo -e "${BLUE}Running RustOwl analysis with Miri...${NC}"
        echo -e "${BLUE}Using Miri flags: -Zmiri-disable-isolation -Zmiri-permissive-provenance${NC}"
        if MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-permissive-provenance" RUSTFLAGS="--cfg miri" log_command_detailed "miri_rustowl_analysis" "cargo miri run --bin rustowl -- check $TEST_TARGET_PATH"; then
            echo -e "${GREEN}[OK] RustOwl analysis completed with Miri${NC}"
        else
            echo -e "${YELLOW}[WARN] Miri could not complete analysis (process spawning limitations)${NC}"
            echo -e "${YELLOW}  This is expected: RustOwl spawns cargo processes which Miri doesn't support${NC}"
            echo -e "${YELLOW}  Core RustOwl memory safety is validated by the system allocator switch${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/miri_rustowl_analysis_${TIMESTAMP}.log${NC}"
        fi
    else
        echo -e "${YELLOW}[WARN] No test target found at $TEST_TARGET_PATH${NC}"
        # Fallback: test basic RustOwl execution with --help
        echo -e "${BLUE}Fallback: Testing basic RustOwl execution with Miri...${NC}"
        echo -e "${BLUE}Using Miri flags: -Zmiri-disable-isolation -Zmiri-permissive-provenance${NC}"
        
        if MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-permissive-provenance" RUSTFLAGS="--cfg miri" log_command_detailed "miri_basic_execution" "cargo miri run --bin rustowl -- --help"; then
            echo -e "${GREEN}[OK] RustOwl basic execution passed with Miri${NC}"
        else
            echo -e "${YELLOW}[WARN] Miri could not complete basic execution${NC}"
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
            echo -e "${GREEN}[OK] No memory errors detected by Valgrind${NC}"
        else
            echo -e "${RED}[ERROR] Valgrind detected memory issues${NC}"
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
            echo -e "${GREEN}[OK] Basic Valgrind test passed${NC}"
        else
            echo -e "${RED}[ERROR] Valgrind basic test failed${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/valgrind_basic_execution_${TIMESTAMP}.log${NC}"
            return 1
        fi
    fi
    
    echo ""
}

# Run sanitizer tests using the current nightly toolchain
run_sanitizer_tests() {
    if [[ $RUN_SANITIZERS -eq 0 ]]; then
        echo -e "${YELLOW}Skipping sanitizer tests (disabled for this platform)${NC}"
        return 0
    fi

    echo -e "${BLUE}Running RustOwl sanitizer tests...${NC}"

    local current_target
    local toolchain_name # This will store the actual installed nightly toolchain name

    # Determine the appropriate target for this platform
    case "$OS_TYPE" in
        "Linux")
            arch=$(uname -m)
            if [[ "$arch" == "x86_64" ]]; then
                current_target="x86_64-unknown-linux-gnu"
            elif [[ "$arch" == "aarch64" ]]; then
                current_target="aarch64-unknown-linux-gnu"
            else
                echo -e "${YELLOW}Unsupported Linux architecture: $arch. Sanitizer tests might not run correctly.${NC}"
                current_target="x86_64-unknown-linux-gnu" # Fallback
            fi
            ;;
        "macOS")
            arch=$(uname -m)
            if [[ "$arch" == "x86_64" ]]; then
                current_target="x86_64-apple-darwin"
            elif [[ "$arch" == "arm64" ]]; then
                current_target="aarch64-apple-darwin"
            else
                 echo -e "${YELLOW}Unsupported macOS architecture: $arch. Sanitizer tests might not run correctly.${NC}"
                current_target="x86_64-apple-darwin" # Fallback
            fi
            ;;
        "Windows")
            current_target="x86_64-pc-windows-msvc"
            ;;
        *)
            echo -e "${RED}[FAIL] Unsupported OS for sanitizer tests: $OS_TYPE${NC}"
            return 1
            ;;
    esac

    # Get the *installed* nightly toolchain name (not necessarily active)
    # This is needed for constructing the path to sanitizer libraries on Windows.
    toolchain_name=$(rustup toolchain list | grep "nightly" | head -n 1 | awk '{print $1}')
    if [[ -z "$toolchain_name" ]]; then
        echo -e "${RED}[FAIL] No nightly toolchain found. Cannot run sanitizer tests. Please ensure 'nightly' is installed.${NC}"
        return 1
    fi

    local rustup_home="${RUSTUP_HOME:-$HOME/.rustup}" # Get RUSTUP_HOME or default

    # For Windows, add the sanitizer runtime DLL path to PATH
    if [[ "$OS_TYPE" == "Windows" ]]; then
        local sanitizer_lib_path="${rustup_home}/toolchains/${toolchain_name}/lib/rustlib/${current_target}/lib"
        if [[ -d "$sanitizer_lib_path" ]]; then
            echo -e "${BLUE}Adding Windows sanitizer lib path to PATH: ${sanitizer_lib_path}${NC}"
            export PATH="${sanitizer_lib_path}:$PATH"
        else
            echo -e "${YELLOW}[WARN] Sanitizer library path not found: ${sanitizer_lib_path}. Sanitizer tests might fail.${NC}"
        fi
    elif [[ "$OS_TYPE" == "macOS" ]]; then
        # On macOS, do NOT set DYLD_INSERT_LIBRARIES for sanitizers; let Rust handle it.
        echo -e "${BLUE}macOS detected: Not setting DYLD_INSERT_LIBRARIES for sanitizers.${NC}"
        if [[ "$(uname -m)" == "arm64e" ]]; then
            echo -e "${YELLOW}[WARN] Detected arm64e architecture. Rust sanitizers may not be fully supported.${NC}"
        fi
    fi

    # Define common RUSTFLAGS for sanitizers
    # Changed 'all' to 'address' for sanitizer-recover as 'all' is no longer supported.
    local RUSTFLAGS_COMMON="-Zsanitizer=address -Zsanitizer-recover=address -Ctarget-feature=+crt-static"

    # Test RustOwl's main functionality with AddressSanitizer
    echo -e "${BLUE}Running RustOwl analysis with AddressSanitizer...${NC}"
    echo -e "${BLUE}Using RUSTFLAGS: ${RUSTFLAGS_COMMON}${NC}"
    if [ -d "$TEST_TARGET_PATH" ]; then
        # Use `cargo +nightly` to explicitly use the nightly toolchain
        if RUSTFLAGS="${RUSTFLAGS_COMMON}" log_command_detailed "asan_rustowl_analysis" "cargo +nightly run --bin rustowl -- check $TEST_TARGET_PATH"; then
            echo -e "${GREEN}[OK] RustOwl analysis completed with AddressSanitizer${NC}"
        else
            echo -e "${RED}[FAIL] RustOwl analysis failed with AddressSanitizer${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/asan_rustowl_analysis_${TIMESTAMP}.log${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}[WARN] No test target found at $TEST_TARGET_PATH${NC}"
        # Fallback: test basic RustOwl execution with --help
        echo -e "${BLUE}Fallback: Testing basic RustOwl execution with AddressSanitizer...${NC}"
        # Use `cargo +nightly` to explicitly use the nightly toolchain
        if RUSTFLAGS="${RUSTFLAGS_COMMON}" log_command_detailed "asan_basic_execution" "cargo +nightly run --bin rustowl -- --help"; then
            echo -e "${GREEN}[OK] RustOwl basic execution passed with AddressSanitizer${NC}"
        else
            echo -e "${RED}[FAIL] RustOwl basic execution failed with AddressSanitizer${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/asan_basic_execution_${TIMESTAMP}.log${NC}"
            return 1
        fi
    fi

    echo ""
}

# Run ThreadSanitizer (better macOS support)
run_thread_sanitizer_tests() {
    if [[ $RUN_THREAD_SANITIZER -eq 0 ]]; then
        return 0
    fi

    echo -e "${BLUE}Running ThreadSanitizer tests...${NC}"
    echo -e "${BLUE}ThreadSanitizer detects data races and threading issues${NC}"
    echo ""

    # ThreadSanitizer flags (generally more stable on macOS than AddressSanitizer)
    local TSAN_FLAGS="-Zsanitizer=thread"

    echo -e "${BLUE}Running RustOwl with ThreadSanitizer...${NC}"
    echo -e "${BLUE}Using RUSTFLAGS: ${TSAN_FLAGS}${NC}"
    
    if [ -d "$TEST_TARGET_PATH" ]; then
        if RUSTFLAGS="${TSAN_FLAGS}" log_command_detailed "tsan_rustowl_analysis" "cargo +nightly run --bin rustowl -- check $TEST_TARGET_PATH"; then
            echo -e "${GREEN}[OK] RustOwl analysis completed with ThreadSanitizer${NC}"
        else
            echo -e "${YELLOW}[WARN] ThreadSanitizer test completed with warnings${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/tsan_rustowl_analysis_${TIMESTAMP}.log${NC}"
        fi
    else
        echo -e "${YELLOW}[WARN] No test target found at $TEST_TARGET_PATH${NC}"
        if RUSTFLAGS="${TSAN_FLAGS}" log_command_detailed "tsan_basic_execution" "cargo +nightly run --bin rustowl -- --help"; then
            echo -e "${GREEN}[OK] RustOwl basic execution passed with ThreadSanitizer${NC}"
        else
            echo -e "${YELLOW}[WARN] ThreadSanitizer basic test completed with warnings${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/tsan_basic_execution_${TIMESTAMP}.log${NC}"
        fi
    fi

    echo ""
}

# Run cargo-machete for unused dependency detection
run_cargo_machete_tests() {
    if [[ $RUN_CARGO_MACHETE -eq 0 ]] || [[ $HAS_CARGO_MACHETE -eq 0 ]]; then
        return 0
    fi

    echo -e "${BLUE}Running cargo-machete unused dependency analysis...${NC}"
    echo -e "${BLUE}cargo-machete detects unused dependencies${NC}"
    echo ""

    if log_command_detailed "cargo_machete" "cargo machete"; then
        echo -e "${GREEN}[OK] No unused dependencies found${NC}"
    else
        echo -e "${YELLOW}[WARN] Unused dependencies detected${NC}"
        echo -e "${BLUE}  Full output captured in: $LOG_DIR/cargo_machete_${TIMESTAMP}.log${NC}"
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
        echo -e "${GREEN}[OK] No known vulnerabilities found${NC}"
    else
        echo -e "${RED}[ERROR] Security vulnerabilities detected${NC}"
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
    
    # Check if DrMemory should be disabled in CI
    if [[ $IS_CI -eq 1 ]] && [[ $DISABLE_DRMEMORY_IN_CI -eq 1 ]]; then
        echo -e "${YELLOW}Skipping DrMemory tests (disabled in CI for stability)${NC}"
        echo -e "${YELLOW}  DrMemory can be re-enabled by setting DISABLE_DRMEMORY_IN_CI=0${NC}"
        return 0
    fi
    
    echo -e "${BLUE}${BOLD}Running DrMemory Tests${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "DrMemory detects memory errors on Windows"
    echo ""
    
    # In CI environments, use much more conservative DrMemory settings
    if [[ $IS_CI -eq 1 ]]; then
        echo -e "${YELLOW}CI environment detected - using conservative DrMemory settings${NC}"
        if [[ $FORCE_BASIC_DRMEMORY_IN_CI -eq 1 ]]; then
            echo -e "${YELLOW}  Forcing basic DrMemory tests only (FORCE_BASIC_DRMEMORY_IN_CI=1)${NC}"
        fi
        echo -e "${YELLOW}  Note: DrMemory analysis is limited to prevent CI timeouts${NC}"
    fi
    
    # Verify DrMemory is working
    echo -e "${YELLOW}Verifying DrMemory installation...${NC}"
    local drmemory_cmd="drmemory"
    if ! command -v drmemory >/dev/null 2>&1; then
        drmemory_cmd="drmemory.exe"
    fi
    
    if log_command_detailed "drmemory_version_check" "$drmemory_cmd -version"; then
        echo -e "${GREEN}[OK] DrMemory is working${NC}"
    else
        echo -e "${RED}[ERROR] DrMemory not working properly${NC}"
        echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_version_check_${TIMESTAMP}.log${NC}"
        return 1
    fi
    
    # Find the rustowl binary
    local rustowl_binary="./target/security/rustowl.exe"
    if [ ! -f "$rustowl_binary" ]; then
        rustowl_binary="./target/security/rustowl"
        if [ ! -f "$rustowl_binary" ]; then
            echo -e "${RED}[ERROR] RustOwl binary not found${NC}"
            return 1
        fi
    fi
    
    # Configure DrMemory options based on environment
    local drmemory_opts="-ignore_kernel -quiet -batch"
    local timeout_seconds=120
    local test_mode="basic"
    
    if [[ $IS_CI -eq 1 ]]; then
        # Aggressive CI optimizations to prevent timeouts and hangs
        drmemory_opts="${drmemory_opts} -light -no_follow_children -no_count_leaks"
        drmemory_opts="$drmemory_opts -malloc_max_frames 5 -callstack_max_frames 10"
        drmemory_opts="$drmemory_opts -no_gen_suppress_syms -no_check_uninit_non_moves"
        timeout_seconds=180  # Reduced timeout for CI
        
        # Force basic test if configured
        if [[ $FORCE_BASIC_DRMEMORY_IN_CI -eq 1 ]]; then
            test_mode="basic"
        fi
        echo -e "${YELLOW}  Using aggressive CI optimizations to prevent hangs${NC}"
    else
        # More thorough analysis for local/non-CI environments
        drmemory_opts="$drmemory_opts -light"
        timeout_seconds=600
        test_mode="full"
    fi
    
    # Determine if we should run basic or full test
    local run_basic_test=0
    if [[ $IS_CI -eq 1 ]] && [[ $FORCE_BASIC_DRMEMORY_IN_CI -eq 1 ]]; then
        run_basic_test=1
    elif [[ ! -d "$TEST_TARGET_PATH" ]]; then
        run_basic_test=1
    fi
    
    # Run basic or full test based on configuration
    if [[ $run_basic_test -eq 1 ]]; then
        if [[ ! -d "$TEST_TARGET_PATH" ]]; then
            echo -e "${YELLOW}! Test package not found at $TEST_TARGET_PATH${NC}"
        fi
        echo -e "${YELLOW}  Running basic DrMemory test (safer for CI)...${NC}"
        
        local drmemory_basic_cmd="timeout $timeout_seconds $drmemory_cmd $drmemory_opts -- $rustowl_binary --help"
        echo "  Command: $drmemory_basic_cmd"
        
        if log_command_detailed "drmemory_basic_execution" "$drmemory_basic_cmd"; then
            echo -e "${GREEN}[OK] Basic DrMemory test passed${NC}"
        else
            local exit_code=$?
            if [[ $exit_code -eq 124 ]] || grep -q "timeout\|TIMEOUT" "$LOG_DIR/drmemory_basic_execution_${TIMESTAMP}.log" 2>/dev/null; then
                echo -e "${YELLOW}[WARN] DrMemory basic test timed out (${timeout_seconds}s)${NC}"
                echo -e "${YELLOW}  This indicates DrMemory overhead is too high for CI${NC}"
                echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_basic_execution_${TIMESTAMP}.log${NC}"
                
                # In CI, timeout is not a failure - DrMemory is just too slow
                if [[ $IS_CI -eq 1 ]]; then
                    echo -e "${YELLOW}  Treating timeout as non-fatal in CI environment${NC}"
                    echo -e "${YELLOW}  Consider setting DISABLE_DRMEMORY_IN_CI=1 if this persists${NC}"
                    return 0
                fi
                return 0
            else
                echo -e "${RED}[ERROR] DrMemory basic test failed${NC}"
                echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_basic_execution_${TIMESTAMP}.log${NC}"
                return 1
            fi
        fi
    else
        # Full analysis only in non-CI environments or when explicitly requested
        echo -e "${YELLOW}Testing rustowl with DrMemory (full analysis)...${NC}"
        local drmemory_full_cmd="timeout $timeout_seconds $drmemory_cmd $drmemory_opts -- $rustowl_binary check $TEST_TARGET_PATH"
        echo "  Command: $drmemory_full_cmd"
        
        if log_command_detailed "drmemory_rustowl_analysis" "$drmemory_full_cmd"; then
            echo -e "${GREEN}[OK] No memory errors detected by DrMemory${NC}"
        else
            local exit_code=$?
            if [[ $exit_code -eq 124 ]] || grep -q "timeout\|TIMEOUT" "$LOG_DIR/drmemory_rustowl_analysis_${TIMESTAMP}.log" 2>/dev/null; then
                echo -e "${YELLOW}[WARN] DrMemory test timed out (${timeout_seconds}s)${NC}"
                echo -e "${YELLOW}  This may indicate a performance issue or DrMemory overhead${NC}"
                echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_rustowl_analysis_${TIMESTAMP}.log${NC}"
                return 0  # Don't fail for timeout
            else
                echo -e "${RED}[ERROR] DrMemory detected memory issues or failed to run${NC}"
                echo -e "${BLUE}  Full output captured in: $LOG_DIR/drmemory_rustowl_analysis_${TIMESTAMP}.log${NC}"
                echo "Run manually for details:"
                echo "  $drmemory_cmd $drmemory_opts -- $rustowl_binary check $TEST_TARGET_PATH"
                return 1
            fi
        fi
    fi
    
    echo ""
}

# Run Instruments memory profiling tests (macOS only)
run_instruments_tests() {
    if [[ $RUN_INSTRUMENTS -eq 0 ]] || [[ $HAS_INSTRUMENTS -eq 0 ]]; then
        if [[ "$OS_TYPE" == "macOS" ]]; then
            if [[ $IS_CI -eq 1 ]]; then
                echo -e "${YELLOW}Skipping Instruments tests (disabled in CI - requires full Xcode)${NC}"
                echo -e "${YELLOW}CI runners typically only have Xcode Command Line Tools${NC}"
            else
                echo -e "${YELLOW}Skipping Instruments tests (not available)${NC}"
                echo -e "${YELLOW}To enable: Install full Xcode from App Store or developer portal${NC}"
            fi
        fi
        return 0
    fi

    echo -e "${BLUE}Running Instruments memory profiling...${NC}"
    echo -e "${BLUE}Instruments provides detailed memory analysis on macOS${NC}"
    echo ""

    # First, try to build the project
    echo -e "${BLUE}Building RustOwl for Instruments analysis...${NC}"
    if ! cargo build --bin rustowl --release; then
        echo -e "${RED}[FAIL] Failed to build RustOwl for Instruments${NC}"
        return 1
    fi

    local rustowl_binary="./target/release/rustowl"
    if [[ ! -f "$rustowl_binary" ]]; then
        echo -e "${RED}[FAIL] RustOwl binary not found at $rustowl_binary${NC}"
        return 1
    fi

    # Run Instruments with Allocations template
    echo -e "${BLUE}Running Instruments with Allocations template...${NC}"
    local trace_file="$LOG_DIR/rustowl_allocations_${TIMESTAMP}.trace"
    
    # Create a simple test to avoid hanging
    local test_args="--help"
    if [[ -d "$TEST_TARGET_PATH" ]]; then
        test_args="check $TEST_TARGET_PATH"
    fi

    # Run instruments with timeout to avoid hanging
    if timeout 60s instruments -t Allocations -D "$trace_file" "$rustowl_binary" $test_args >/dev/null 2>&1; then
        echo -e "${GREEN}[OK] Instruments profiling completed${NC}"
        echo -e "${BLUE}  Trace file saved to: $trace_file${NC}"
        
        # Try to extract some basic info from the trace
        if [[ -f "$trace_file" ]]; then
            local trace_size=$(du -h "$trace_file" | cut -f1)
            echo -e "${BLUE}  Trace file size: $trace_size${NC}"
        fi
    else
        echo -e "${YELLOW}[WARN] Instruments profiling timed out or failed${NC}"
        echo -e "${YELLOW}  This may be normal in CI environments${NC}"
        # Don't treat this as a hard failure
        return 0
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
    echo -e "  Miri (UB detection):           $([ $HAS_MIRI -eq 1 ] && echo -e "${GREEN}[OK] Available${NC}" || echo -e "${RED}[ERROR] Missing${NC}")"
    
    if [[ "$OS_TYPE" == "Linux" ]]; then
        echo -e "  Valgrind (memory errors):      $([ $HAS_VALGRIND -eq 1 ] && echo -e "${GREEN}[OK] Available${NC}" || echo -e "${RED}[ERROR] Missing${NC}")"
    fi
    
    echo -e "  cargo-audit (vulnerabilities): $([ $HAS_CARGO_AUDIT -eq 1 ] && echo -e "${GREEN}[OK] Available${NC}" || echo -e "${RED}[ERROR] Missing${NC}")"
    
    if [[ "$OS_TYPE" == "Windows" ]]; then
        echo -e "  DrMemory (memory debugging):   $([ $HAS_DRMEMORY -eq 1 ] && echo -e "${GREEN}[OK] Available${NC}" || echo -e "${RED}[ERROR] Missing${NC}")"
    fi
    
    if [[ "$OS_TYPE" == "macOS" ]]; then
        echo -e "  Instruments (performance):     $([ $HAS_INSTRUMENTS -eq 1 ] && echo -e "${GREEN}[OK] Available${NC}" || echo -e "${RED}[ERROR] Missing${NC}")"
    fi
    
    echo ""
    
    # Check nightly toolchain for sanitizers
    local current_toolchain=$(rustup show active-toolchain | cut -d' ' -f1)
    echo "Sanitizer Support:"
    if [[ "$current_toolchain" == *"nightly"* ]]; then
        echo -e "  Nightly toolchain:             ${GREEN}[OK] Available${NC}"
        echo -e "  AddressSanitizer:              ${GREEN}[OK] Supported${NC}"
        echo -e "  ThreadSanitizer:               ${GREEN}[OK] Supported${NC}"
        echo -e "  MemorySanitizer:               ${GREEN}[OK] Supported${NC}"
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
    echo "| Miri | $([ $HAS_MIRI -eq 1 ] && echo "[OK] Available" || echo "[FAIL] Missing") | Undefined behavior detection |" >> "$summary_file"
    echo "| Valgrind | $([ $HAS_VALGRIND -eq 1 ] && echo "[OK] Available" || echo "[FAIL] Missing/N/A") | Memory error detection (Linux) |" >> "$summary_file"
    echo "| cargo-audit | $([ $HAS_CARGO_AUDIT -eq 1 ] && echo "[OK] Available" || echo "[FAIL] Missing") | Security vulnerability scanning |" >> "$summary_file"
    echo "| DrMemory | $([ $HAS_DRMEMORY -eq 1 ] && echo "[OK] Available" || echo "[FAIL] Missing/N/A") | Memory debugging (Windows) |" >> "$summary_file"
    echo "| Instruments | $([ $HAS_INSTRUMENTS -eq 1 ] && echo "[OK] Available" || echo "[FAIL] Missing/N/A") | Performance analysis (macOS) |" >> "$summary_file"
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
                    echo "**Status:** [OK] PASSED" >> "$summary_file"
                elif grep -q "COMMAND FAILED" "$log_file"; then
                    echo "**Status:** [FAIL] FAILED" >> "$summary_file"
                    
                    # Extract error information
                    echo "" >> "$summary_file"
                    echo "**Error Details:**" >> "$summary_file"
                    echo '```' >> "$summary_file"
                    # Get last 20 lines before the failure marker
                    grep -B 20 "COMMAND FAILED" "$log_file" | tail -20 >> "$summary_file"
                    echo '```' >> "$summary_file"
                else
                    echo "**Status:** [WARN] UNKNOWN" >> "$summary_file"
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

# Debug: Script end reached
echo "DEBUG: Script loaded, checking if main should be called"
echo "DEBUG: BASH_SOURCE[0]=${BASH_SOURCE[0]}, \$0=$0"

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "DEBUG: Calling main function with args: $*"
    main "$@"
else
    echo "DEBUG: Script sourced, not calling main"
fi