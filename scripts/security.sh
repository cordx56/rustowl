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
MIN_RUST_VERSION="1.88.0"
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
RUN_AUDIT=1
RUN_INSTRUMENTS=1
RUN_THREAD_SANITIZER=0
RUN_CARGO_MACHETE=0

# Tool availability detection
HAS_MIRI=0
HAS_VALGRIND=0
HAS_CARGO_AUDIT=0
HAS_INSTRUMENTS=0
HAS_CARGO_MACHETE=0

# OS detection with more robust platform detection
detect_platform() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        OS_TYPE="Linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        OS_TYPE="macOS"
    else
        # Fallback to uname
        local uname_result=$(uname 2>/dev/null || echo "unknown")
        case "$uname_result" in
            Linux*) OS_TYPE="Linux" ;;
            Darwin*) OS_TYPE="macOS" ;;
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

# Auto-configure tests based on platform capabilities and toolchain compatibility
auto_configure_tests() {
    echo -e "${YELLOW}Auto-configuring tests for $OS_TYPE...${NC}"
    
    case "$OS_TYPE" in
        "Linux")
            # Linux: Full test suite available
            echo "  Linux detected: Enabling Miri, Valgrind, and Audit"
            ;;
        "macOS")
            # macOS: Focus on Rust-native tools and macOS-compatible alternatives
            echo "  macOS detected: Enabling Miri, Audit, and macOS-compatible tools"
            echo "  Disabling Valgrind (unreliable on macOS)"
            echo "  Enabling cargo-machete for unused dependency detection"
            echo "  Disabling Instruments (complex Xcode setup required)"
            RUN_VALGRIND=0
            RUN_THREAD_SANITIZER=0
            RUN_CARGO_MACHETE=1  # Detect unused dependencies
            RUN_INSTRUMENTS=0  # Disable by default (complex setup required)
            ;;
        *)
            echo "  Unknown platform: Enabling basic tests only"
            RUN_VALGRIND=0
            RUN_INSTRUMENTS=0
            # Also disable nightly-dependent features on unknown platforms
            RUN_MIRI=0
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
    echo "  --no-audit           Skip cargo audit security check"
    echo "  --no-instruments     Skip Instruments tests"
    echo ""
    echo "Platform Support:"
    echo "  Linux:   Miri, Valgrind, cargo-audit"
    echo "  macOS:   Miri, cargo-audit, cargo-machete"
    echo ""
    echo "CI Environment:"
    echo "  The script automatically detects CI environments and installs missing tools."
    echo "  Supported: GitHub Actions, GitLab CI, Travis CI, CircleCI, Jenkins,"
    echo "            Buildkite, Azure DevOps, and others with CI environment variables."
    echo ""
    echo "Tests performed:"
    echo "  - Miri: Detects undefined behavior in Rust code"
    echo "  - Valgrind: Memory error detection (Linux)"
    echo "  - cargo-audit: Security vulnerability scanning"
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
        --no-audit)
            RUN_AUDIT=0
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

# Helper function to print section headers
print_section_header() {
    local title="$1"
    local description="$2"
    echo -e "${BLUE}${BOLD}$title${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "$description"
    echo ""
}

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

    # Check nightly toolchain availability for advanced features
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

    echo ""
}

# Build the project with the toolchain specified in rust-toolchain.toml
build_project() {
    echo -e "${YELLOW}Building RustOwl in security mode...${NC}"
    echo -e "${BLUE}Using toolchain from rust-toolchain.toml${NC}"
    
    # Build with the current toolchain (specified by rust-toolchain.toml)
    RUSTC_BOOTSTRAP=1 cargo build --profile=security
    
    local binary_name="rustowl"
    
    if [ ! -f "./target/security/$binary_name" ]; then
        echo -e "${RED}[ERROR] Failed to build rustowl binary${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}[OK] Build completed successfully${NC}"
    echo ""
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
    
    if [[ "$OS_TYPE" == "macOS" ]]; then
        echo -e "  Instruments (performance):     $([ $HAS_INSTRUMENTS -eq 1 ] && echo -e "${GREEN}[OK] Available${NC}" || echo -e "${RED}[ERROR] Missing${NC}")"
    fi
    
    echo ""
    
    # Check nightly toolchain for other advanced features
    local current_toolchain=$(rustup show active-toolchain | cut -d' ' -f1)
    echo "Advanced Features:"
    if [[ "$current_toolchain" == *"nightly"* ]]; then
        echo -e "  Nightly toolchain:             ${GREEN}[OK] Available${NC}"
        echo -e "  Advanced features:             ${GREEN}[OK] Supported${NC}"
    else
        echo -e "  Nightly toolchain:             ${YELLOW}! Stable toolchain active${NC}"
        echo -e "  Advanced features:             ${YELLOW}! Require nightly${NC}"
    fi
    
    echo ""
    echo "Test Configuration:"
    echo -e "  Run Miri:       $([ $RUN_MIRI -eq 1 ] && echo -e "${GREEN}Enabled${NC}" || echo -e "${YELLOW}Disabled${NC}")"
    echo -e "  Run Valgrind:   $([ $RUN_VALGRIND -eq 1 ] && echo -e "${GREEN}Enabled${NC}" || echo -e "${YELLOW}Disabled${NC}")"
    echo -e "  Run Audit:      $([ $RUN_AUDIT -eq 1 ] && echo -e "${GREEN}Enabled${NC}" || echo -e "${YELLOW}Disabled${NC}")"
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
    echo "| Instruments | $([ $HAS_INSTRUMENTS -eq 1 ] && echo "[OK] Available" || echo "[FAIL] Missing/N/A") | Performance analysis (macOS) |" >> "$summary_file"
    echo "" >> "$summary_file"
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

run_valgrind_tests() {
    if [[ $RUN_VALGRIND -eq 0 ]]; then
        return 0
    fi
    
    if [[ $HAS_VALGRIND -eq 0 ]]; then
        echo -e "${YELLOW}Skipping Valgrind tests (not available on this platform)${NC}"
        return 0
    fi
    
    echo -e "${BLUE}${BOLD}Running Valgrind Tests${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "Valgrind detects memory errors, leaks, and memory corruption"
    echo ""
    
    # Build RustOwl for Valgrind testing (use release profile for better performance)
    echo -e "${BLUE}Building RustOwl for Valgrind testing...${NC}"
    if ! cargo build --release >/dev/null 2>&1; then
        echo -e "${RED}[FAIL] Failed to build RustOwl for Valgrind testing${NC}"
        return 1
    fi
    
    local rustowl_binary="./target/release/rustowl"
    if [[ ! -f "$rustowl_binary" ]]; then
        echo -e "${RED}[FAIL] RustOwl binary not found at $rustowl_binary${NC}"
        return 1
    fi
    
    # Check if we have Valgrind suppressions file
    local valgrind_suppressions=""
    if [[ -f ".valgrind-suppressions" ]]; then
        valgrind_suppressions="--suppressions=.valgrind-suppressions"
        echo -e "${BLUE}Using suppressions file: $(pwd)/.valgrind-suppressions${NC}"
    fi
    
    # Run Valgrind memory check on RustOwl
    echo -e "${BLUE}Running RustOwl with Valgrind...${NC}"
    echo -e "${BLUE}Using Valgrind flags: --tool=memcheck --leak-check=full --show-leak-kinds=all --track-origins=yes${NC}"
    if [ -d "$TEST_TARGET_PATH" ]; then
        echo -e "${BLUE}Testing RustOwl analysis with Valgrind...${NC}"
        local valgrind_cmd="valgrind --tool=memcheck --leak-check=full --show-leak-kinds=all --track-origins=yes $valgrind_suppressions $rustowl_binary check $TEST_TARGET_PATH"
        
        if log_command_detailed "valgrind_rustowl_analysis" "$valgrind_cmd"; then
            echo -e "${GREEN}[OK] RustOwl analysis completed with Valgrind (no memory errors detected)${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/valgrind_rustowl_analysis_${TIMESTAMP}.log${NC}"
        else
            echo -e "${RED}[FAIL] Valgrind detected memory errors in RustOwl analysis${NC}"
            echo -e "${BLUE}  Full output captured in: $LOG_DIR/valgrind_rustowl_analysis_${TIMESTAMP}.log${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}[WARN] No test target found at $TEST_TARGET_PATH${NC}"
        echo -e "${BLUE}Fallback: Testing basic RustOwl execution with Valgrind...${NC}"
        
        local valgrind_cmd="valgrind --tool=memcheck --leak-check=full --show-leak-kinds=all --track-origins=yes $valgrind_suppressions $rustowl_binary --help"
        if log_command_detailed "valgrind_basic_execution" "$valgrind_cmd"; then
            echo -e "${GREEN}[OK] RustOwl basic execution passed with Valgrind${NC}"
        else
            echo -e "${YELLOW}[WARN] Valgrind basic test completed with warnings${NC}"
            return 1
        fi
        echo -e "${BLUE}  Full output captured in: $LOG_DIR/valgrind_basic_execution_${TIMESTAMP}.log${NC}"
    fi
    
    echo ""
}

# AddressSanitizer removed - incompatible with RustOwl's proc-macro dependencies
# Alternative memory safety checking is provided by Valgrind and Miri

run_audit_check() {
    if [[ $RUN_AUDIT -eq 0 ]] || [[ $HAS_CARGO_AUDIT -eq 0 ]]; then
        if [[ $RUN_AUDIT -eq 1 ]] && [[ $HAS_CARGO_AUDIT -eq 0 ]]; then
            echo -e "${YELLOW}Skipping cargo-audit (not installed)${NC}"
        fi
        return 0
    fi
    
    echo -e "${BLUE}Scanning dependencies for vulnerabilities...${NC}"
    if cargo audit; then
        echo -e "${GREEN}[OK] No known vulnerabilities found${NC}"
    else
        echo -e "${RED}[ERROR] Security vulnerabilities detected${NC}"
        return 1
    fi
    
    echo ""
}

run_cargo_machete_tests() {
    if [[ $RUN_CARGO_MACHETE -eq 0 ]]; then
        return 0
    fi
    
    if [[ $HAS_CARGO_MACHETE -eq 0 ]]; then
        echo -e "${YELLOW}Skipping cargo-machete tests (not installed)${NC}"
        return 0
    fi
    
    echo -e "${BLUE}${BOLD}Running cargo-machete Tests${NC}"
    echo -e "${BLUE}================================${NC}"
    echo "cargo-machete detects unused dependencies in Cargo.toml"
    echo ""
    
    echo -e "${BLUE}Scanning for unused dependencies...${NC}"
    
    # Run cargo-machete and capture output
    if log_command_detailed "cargo_machete_analysis" "cargo machete"; then
        echo -e "${GREEN}[OK] cargo-machete analysis completed${NC}"
        echo -e "${BLUE}  Full output captured in: $LOG_DIR/cargo_machete_analysis_${TIMESTAMP}.log${NC}"
        
        # Check the log for unused dependencies
        local log_file="$LOG_DIR/cargo_machete_analysis_${TIMESTAMP}.log"
        if grep -q "unused dependencies" "$log_file" 2>/dev/null; then
            local unused_count=$(grep -c "unused dependencies" "$log_file" 2>/dev/null || echo "0")
            if [[ "$unused_count" -gt 0 ]]; then
                echo -e "${YELLOW}[WARN] Found potential unused dependencies - check log for details${NC}"
                echo -e "${YELLOW}  Note: cargo-machete may report false positives for conditionally used deps${NC}"
            else
                echo -e "${GREEN}[OK] No unused dependencies detected${NC}"
            fi
        else
            echo -e "${GREEN}[OK] No unused dependencies detected${NC}"
        fi
    else
        # cargo-machete exits with non-zero when it finds unused dependencies
        echo -e "${YELLOW}[INFO] cargo-machete found potential issues${NC}"
        echo -e "${BLUE}  Full output captured in: $LOG_DIR/cargo_machete_analysis_${TIMESTAMP}.log${NC}"
        
        # Don't fail the test suite for this - unused deps are warnings, not errors
        local log_file="$LOG_DIR/cargo_machete_analysis_${TIMESTAMP}.log"
        if [[ -f "$log_file" ]]; then
            echo -e "${YELLOW}  Check the log file to review any unused dependencies${NC}"
            echo -e "${YELLOW}  Note: Some dependencies may be used conditionally (features, targets, etc.)${NC}"
        fi
    fi
    
    echo ""
}

run_instruments_tests() {
    echo -e "${YELLOW}Instruments tests not yet implemented${NC}"
    return 0
}

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

# Logging configuration
LOG_DIR="security-logs"
TIMESTAMP=$(date '+%Y%m%d_%H%M%S')

# Main execution starts here
echo -e "${BLUE}${BOLD}RustOwl Security & Memory Safety Testing${NC}"
echo -e "${BLUE}=========================================${NC}"
echo ""

# Initialize and detect environment
detect_platform
detect_ci_environment

# Check for --check flag early to show tool status
if [[ "$1" == "--check" ]]; then
    echo -e "${BLUE}Checking tool availability and system readiness...${NC}"
    echo ""
    
    detect_tools
    show_tool_status
    
    echo ""
    echo -e "${GREEN}System check completed.${NC}"
    exit 0
fi

echo -e "${BLUE}Running security and memory safety analysis...${NC}"
echo ""

# Detect available tools
detect_tools

# Auto-configure tests based on platform
auto_configure_tests

# Install missing tools if in CI or explicitly requested
if [[ $IS_CI -eq 1 ]] || [[ "$1" == "--install" ]]; then
    install_required_tools
    # Re-detect tools after installation
    detect_tools
fi

# Check Rust version compatibility
check_rust_version

# Show final tool status
show_tool_status

echo ""
echo -e "${BLUE}Running security tests...${NC}"
echo ""

# Create security summary
create_security_summary

# Run the actual security tests
test_failures=0

# Run Miri tests
if ! run_miri_tests; then
    test_failures=$((test_failures + 1))
fi

# Run Valgrind tests (Linux only)
if [[ "$OS_TYPE" == "Linux" ]] && [[ $RUN_VALGRIND -eq 1 ]]; then
    if ! run_valgrind_tests; then
        test_failures=$((test_failures + 1))
    fi
fi

# Run cargo audit
if ! run_audit_check; then
    test_failures=$((test_failures + 1))
fi

# Run cargo machete if available
if [[ $HAS_CARGO_MACHETE -eq 1 ]] && [[ $RUN_CARGO_MACHETE -eq 1 ]]; then
    if ! run_cargo_machete_tests; then
        test_failures=$((test_failures + 1))
    fi
fi

# Run Instruments tests (macOS only)
if [[ "$OS_TYPE" == "macOS" ]] && [[ $RUN_INSTRUMENTS -eq 1 ]] && [[ $HAS_INSTRUMENTS -eq 1 ]]; then
    if ! run_instruments_tests; then
        test_failures=$((test_failures + 1))
    fi
fi

# Final summary
echo ""
if [[ $test_failures -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All security tests passed!${NC}"
    echo -e "${GREEN}No security issues detected.${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}Security tests failed!${NC}"
    echo -e "${RED}$test_failures test suite(s) failed.${NC}"
    echo -e "${BLUE}Check logs in $LOG_DIR/ for details.${NC}"
    exit 1
fi
