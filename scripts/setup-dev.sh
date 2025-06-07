#!/usr/bin/env bash

set -euo pipefail

# RustOwl Development Environment Setup Script
# Sets up all tools and dependencies needed for local development

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

# Configuration
RUST_VERSION="1.87.0"
NODE_VERSION="18"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# OS detection
OS=$(uname)

show_help() {
    echo "RustOwl Development Environment Setup"
    echo ""
    echo "USAGE:"
    echo "    $0 [OPTIONS]"
    echo ""
    echo "OPTIONS:"
    echo "    -h, --help         Show this help message"
    echo "    --check-only       Only check what's installed, don't install anything"
    echo "    --rust-only        Only setup Rust toolchain and components"
    echo "    --node-only        Only setup Node.js and yarn"
    echo "    --tools-only       Only setup additional tools (valgrind, bc, etc.)"
    echo "    --skip-rust        Skip Rust setup"
    echo "    --skip-node        Skip Node.js setup"
    echo "    --skip-tools       Skip additional tools setup"
    echo ""
    echo "TOOLS INSTALLED:"
    echo "  Rust Toolchain:"
    echo "    • Rust $RUST_VERSION (stable)"
    echo "    • Nightly toolchain (for sanitizers)"
    echo "    • rustfmt (code formatting)"
    echo "    • clippy (linting)"
    echo "    • miri (undefined behavior detection)"
    echo ""
    echo "  Cargo Tools:"
    echo "    • cargo-audit (security vulnerability scanning)"
    echo "    • cargo-criterion (advanced benchmarking)"
    echo ""
    echo "  Node.js Ecosystem (for VS Code extension):"
    echo "    • Node.js $NODE_VERSION"
    echo "    • yarn package manager"
    echo ""
    echo "  System Tools:"
    echo "    • bc (basic calculator for calculations)"
    echo "    • valgrind (memory debugging, Linux only)"
    echo "    • gnuplot (benchmark plotting, optional)"
    echo ""
    echo "EXAMPLES:"
    echo "    $0                 # Full setup"
    echo "    $0 --check-only    # Check current installation status"
    echo "    $0 --rust-only     # Setup only Rust toolchain"
    echo "    $0 --skip-node     # Setup everything except Node.js"
}

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_header() {
    echo -e "${BLUE}${BOLD}$1${NC}"
    echo -e "${BLUE}$(printf '=%.0s' $(seq 1 ${#1}))${NC}"
}

# Check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check if running as root (not recommended)
check_root() {
    if [ "$EUID" -eq 0 ]; then
        log_warning "Running as root is not recommended for development setup"
        log_warning "Consider running as a regular user with sudo access"
        read -p "Continue anyway? (y/n): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
}

# Detect package manager
detect_package_manager() {
    if command_exists apt-get; then
        echo "apt"
    elif command_exists yum; then
        echo "yum"
    elif command_exists dnf; then
        echo "dnf"
    elif command_exists pacman; then
        echo "pacman"
    elif command_exists brew; then
        echo "brew"
    elif command_exists zypper; then
        echo "zypper"
    else
        echo "unknown"
    fi
}

# Install system packages
install_system_packages() {
    local pkg_manager
    pkg_manager=$(detect_package_manager)
    
    log_info "Detected package manager: $pkg_manager"
    
    case $pkg_manager in
        apt)
            log_info "Installing system packages with apt..."
            sudo apt-get update
            sudo apt-get install -y build-essential curl bc
            if [ "$OS" = "Linux" ]; then
                sudo apt-get install -y valgrind
            fi
            # Optional: gnuplot for benchmark charts
            if ! command_exists gnuplot; then
                log_info "Installing gnuplot (optional, for benchmark charts)..."
                sudo apt-get install -y gnuplot || log_warning "Failed to install gnuplot (optional)"
            fi
            ;;
        yum|dnf)
            local cmd="yum"
            if [ "$pkg_manager" = "dnf" ]; then
                cmd="dnf"
            fi
            log_info "Installing system packages with $cmd..."
            sudo $cmd install -y gcc gcc-c++ curl bc
            if [ "$OS" = "Linux" ]; then
                sudo $cmd install -y valgrind
            fi
            if ! command_exists gnuplot; then
                sudo $cmd install -y gnuplot || log_warning "Failed to install gnuplot (optional)"
            fi
            ;;
        pacman)
            log_info "Installing system packages with pacman..."
            sudo pacman -Sy --noconfirm base-devel curl bc
            if [ "$OS" = "Linux" ]; then
                sudo pacman -S --noconfirm valgrind
            fi
            if ! command_exists gnuplot; then
                sudo pacman -S --noconfirm gnuplot || log_warning "Failed to install gnuplot (optional)"
            fi
            ;;
        zypper)
            log_info "Installing system packages with zypper..."
            sudo zypper install -y gcc gcc-c++ curl bc
            if [ "$OS" = "Linux" ]; then
                sudo zypper install -y valgrind
            fi
            if ! command_exists gnuplot; then
                sudo zypper install -y gnuplot || log_warning "Failed to install gnuplot (optional)"
            fi
            ;;
        brew)
            log_info "Installing system packages with brew..."
            brew install bc gnuplot
            ;;
        *)
            log_warning "Unknown package manager. Please install manually:"
            log_warning "  - build tools (gcc, make, etc.)"
            log_warning "  - curl"
            log_warning "  - bc (basic calculator)"
            if [ "$OS" = "Linux" ]; then
                log_warning "  - valgrind (memory debugging)"
            fi
            log_warning "  - gnuplot (optional, for benchmark charts)"
            ;;
    esac
}

# Setup Rust toolchain
setup_rust() {
    log_header "Setting up Rust toolchain"
    
    # Install rustup if not present
    if ! command_exists rustup; then
        log_info "Installing rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    else
        log_success "rustup already installed"
    fi
    
    # Install specific Rust version
    log_info "Installing Rust $RUST_VERSION..."
    rustup install "$RUST_VERSION"
    rustup default "$RUST_VERSION"
    
    # Install nightly for sanitizers
    log_info "Installing nightly toolchain for sanitizers..."
    rustup install nightly
    
    # Install components
    log_info "Installing Rust components..."
    rustup component add rustfmt clippy
    rustup component add miri --toolchain "$RUST_VERSION"
    
    # Install cargo tools
    log_info "Installing cargo tools..."
    
    # Check if cargo-audit is installed
    if ! command_exists cargo-audit; then
        log_info "Installing cargo-audit..."
        cargo install cargo-audit
    else
        log_success "cargo-audit already installed"
    fi
    
    # Check if cargo-criterion is installed (optional but recommended)
    if ! command_exists cargo-criterion; then
        log_info "Installing cargo-criterion (optional but recommended)..."
        cargo install cargo-criterion || log_warning "Failed to install cargo-criterion (optional)"
    else
        log_success "cargo-criterion already installed"
    fi
    
    log_success "Rust toolchain setup complete"
    echo ""
}

# Setup Node.js and yarn
setup_node() {
    log_header "Setting up Node.js and yarn"
    
    # Check if Node.js is already installed with correct version
    if command_exists node; then
        local current_version
        current_version=$(node --version | sed 's/v//' | cut -d. -f1)
        if [ "$current_version" -ge "$NODE_VERSION" ]; then
            log_success "Node.js $current_version is already installed (>= $NODE_VERSION required)"
        else
            log_warning "Node.js $current_version is installed but < $NODE_VERSION required"
            log_info "Please update Node.js to version $NODE_VERSION or higher"
        fi
    else
        log_info "Installing Node.js..."
        case $(detect_package_manager) in
            apt)
                curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | sudo -E bash -
                sudo apt-get install -y nodejs
                ;;
            yum|dnf)
                local cmd="yum"
                if [ "$(detect_package_manager)" = "dnf" ]; then
                    cmd="dnf"
                fi
                curl -fsSL https://rpm.nodesource.com/setup_${NODE_VERSION}.x | sudo bash -
                sudo $cmd install -y nodejs
                ;;
            pacman)
                sudo pacman -S --noconfirm nodejs npm
                ;;
            brew)
                brew install node@${NODE_VERSION}
                ;;
            *)
                log_warning "Please install Node.js $NODE_VERSION manually from https://nodejs.org/"
                ;;
        esac
    fi
    
    # Install yarn
    if ! command_exists yarn; then
        log_info "Installing yarn..."
        if command_exists npm; then
            npm install -g yarn
        else
            log_warning "npm not found, please install yarn manually"
        fi
    else
        log_success "yarn already installed"
    fi
    
    # Setup VS Code extension dependencies if the directory exists
    if [ -d "vscode" ]; then
        log_info "Installing VS Code extension dependencies..."
        cd vscode
        if [ -f "package.json" ]; then
            yarn install --frozen-lockfile
            log_success "VS Code extension dependencies installed"
        else
            log_warning "No package.json found in vscode directory"
        fi
        cd "$REPO_ROOT"
    else
        log_info "VS Code extension directory not found, skipping"
    fi
    
    log_success "Node.js and yarn setup complete"
    echo ""
}

# Check installation status
check_installation() {
    log_header "Development Environment Status"
    
    echo "Rust Toolchain:"
    if command_exists rustc; then
        local rust_version
        rust_version=$(rustc --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
        echo -e "  • Rust: ${GREEN}✓${NC} $rust_version"
        
        if rustup toolchain list | grep -q nightly; then
            echo -e "  • Nightly: ${GREEN}✓${NC} installed"
        else
            echo -e "  • Nightly: ${RED}✗${NC} not installed"
        fi
        
        if rustup component list --installed | grep -q rustfmt; then
            echo -e "  • rustfmt: ${GREEN}✓${NC} installed"
        else
            echo -e "  • rustfmt: ${RED}✗${NC} not installed"
        fi
        
        if rustup component list --installed | grep -q clippy; then
            echo -e "  • clippy: ${GREEN}✓${NC} installed"
        else
            echo -e "  • clippy: ${RED}✗${NC} not installed"
        fi
        
        if rustup component list --installed | grep -q miri; then
            echo -e "  • miri: ${GREEN}✓${NC} installed"
        else
            echo -e "  • miri: ${RED}✗${NC} not installed"
        fi
    else
        echo -e "  • Rust: ${RED}✗${NC} not installed"
    fi
    
    echo ""
    echo "Cargo Tools:"
    if command_exists cargo-audit; then
        echo -e "  • cargo-audit: ${GREEN}✓${NC} installed"
    else
        echo -e "  • cargo-audit: ${RED}✗${NC} not installed"
    fi
    
    if command_exists cargo-criterion; then
        echo -e "  • cargo-criterion: ${GREEN}✓${NC} installed"
    else
        echo -e "  • cargo-criterion: ${YELLOW}!${NC} not installed (optional)"
    fi
    
    echo ""
    echo "Node.js Ecosystem:"
    if command_exists node; then
        local node_version
        node_version=$(node --version)
        echo -e "  • Node.js: ${GREEN}✓${NC} $node_version"
    else
        echo -e "  • Node.js: ${RED}✗${NC} not installed"
    fi
    
    if command_exists yarn; then
        local yarn_version
        yarn_version=$(yarn --version 2>/dev/null || echo "unknown")
        echo -e "  • yarn: ${GREEN}✓${NC} $yarn_version"
    else
        echo -e "  • yarn: ${RED}✗${NC} not installed"
    fi
    
    echo ""
    echo "System Tools:"
    if command_exists bc; then
        echo -e "  • bc: ${GREEN}✓${NC} installed"
    else
        echo -e "  • bc: ${RED}✗${NC} not installed"
    fi
    
    if [ "$OS" = "Linux" ] && command_exists valgrind; then
        local valgrind_version
        valgrind_version=$(valgrind --version | head -1)
        echo -e "  • valgrind: ${GREEN}✓${NC} $valgrind_version"
    elif [ "$OS" = "Linux" ]; then
        echo -e "  • valgrind: ${RED}✗${NC} not installed"
    else
        echo -e "  • valgrind: ${YELLOW}N/A${NC} (Linux only)"
    fi
    
    if command_exists gnuplot; then
        echo -e "  • gnuplot: ${GREEN}✓${NC} installed"
    else
        echo -e "  • gnuplot: ${YELLOW}!${NC} not installed (optional)"
    fi
    
    echo ""
    echo "VS Code Extension:"
    if [ -d "vscode" ] && [ -d "vscode/node_modules" ]; then
        echo -e "  • Dependencies: ${GREEN}✓${NC} installed"
    elif [ -d "vscode" ]; then
        echo -e "  • Dependencies: ${RED}✗${NC} not installed"
    else
        echo -e "  • VS Code ext: ${YELLOW}N/A${NC} (directory not found)"
    fi
    
    echo ""
}

# Validate scripts functionality
validate_scripts() {
    log_header "Validating script functionality"
    
    local script_dir="$REPO_ROOT/scripts"
    local failed_validations=0
    
    # Check if all scripts exist and are executable
    local scripts=("bench.sh" "security.sh" "dev-checks.sh" "size-check.sh")
    for script in "${scripts[@]}"; do
        local script_path="$script_dir/$script"
        if [ -f "$script_path" ]; then
            if [ -x "$script_path" ]; then
                echo -e "  • $script: ${GREEN}✓${NC} exists and executable"
            else
                echo -e "  • $script: ${YELLOW}!${NC} exists but not executable"
                chmod +x "$script_path"
                echo -e "    Fixed: made executable"
            fi
        else
            echo -e "  • $script: ${RED}✗${NC} not found"
            ((failed_validations++))
        fi
    done
    
    # Test help options for each script
    echo ""
    log_info "Testing help options..."
    for script in "${scripts[@]}"; do
        local script_path="$script_dir/$script"
        if [ -f "$script_path" ] && [ -x "$script_path" ]; then
            if "$script_path" --help >/dev/null 2>&1; then
                echo -e "  • $script --help: ${GREEN}✓${NC}"
            else
                echo -e "  • $script --help: ${RED}✗${NC}"
                ((failed_validations++))
            fi
        fi
    done
    
    if [ $failed_validations -eq 0 ]; then
        log_success "All script validations passed"
    else
        log_error "$failed_validations script validation(s) failed"
    fi
    
    echo ""
}

# Create necessary directories
setup_directories() {
    log_info "Creating necessary directories..."
    
    # Create baselines directory (ignored by git)
    mkdir -p baselines/performance
    mkdir -p baselines
    
    # Ensure gitignore includes baselines
    if [ -f ".gitignore" ]; then
        if ! grep -q "^baselines/" .gitignore; then
            echo "baselines/" >> .gitignore
            log_info "Added baselines/ to .gitignore"
        fi
    fi
    
    log_success "Directories created"
    echo ""
}

main() {
    local check_only=false
    local rust_only=false
    local node_only=false
    local tools_only=false
    local skip_rust=false
    local skip_node=false
    local skip_tools=false
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            --check-only)
                check_only=true
                shift
                ;;
            --rust-only)
                rust_only=true
                shift
                ;;
            --node-only)
                node_only=true
                shift
                ;;
            --tools-only)
                tools_only=true
                shift
                ;;
            --skip-rust)
                skip_rust=true
                shift
                ;;
            --skip-node)
                skip_node=true
                shift
                ;;
            --skip-tools)
                skip_tools=true
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    echo -e "${BLUE}${BOLD}========================================${NC}"
    echo -e "${BLUE}${BOLD}  RustOwl Development Environment Setup${NC}"
    echo -e "${BLUE}${BOLD}========================================${NC}"
    echo ""
    
    if [ "$check_only" = true ]; then
        check_installation
        validate_scripts
        exit 0
    fi
    
    check_root
    
    # Setup based on options
    if [ "$rust_only" = true ]; then
        setup_rust
    elif [ "$node_only" = true ]; then
        setup_node
    elif [ "$tools_only" = true ]; then
        install_system_packages
    else
        # Full setup
        if [ "$skip_tools" = false ]; then
            install_system_packages
        fi
        
        if [ "$skip_rust" = false ]; then
            setup_rust
        fi
        
        if [ "$skip_node" = false ]; then
            setup_node
        fi
    fi
    
    setup_directories
    
    log_header "Setup Complete!"
    check_installation
    validate_scripts
    
    echo -e "${GREEN}${BOLD}Development environment is ready!${NC}"
    echo ""
    echo -e "${BLUE}Next steps:${NC}"
    echo "  1. Run './scripts/dev-checks.sh' to verify everything works"
    echo "  2. Run './scripts/bench.sh --save main' to create performance baseline"
    echo "  3. Run './scripts/security.sh --check' to verify security tools"
    echo ""
    echo -e "${BLUE}Available scripts:${NC}"
    echo "  • ./scripts/dev-checks.sh      - Development checks and fixes"
    echo "  • ./scripts/bench.sh           - Performance benchmarking"
    echo "  • ./scripts/security.sh        - Security and memory safety testing"
    echo "  • ./scripts/size-check.sh      - Binary size monitoring"
    echo "  • ./scripts/bump.sh           - Version management"
}

main "$@"
