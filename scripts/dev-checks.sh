#!/usr/bin/env bash

set -euo pipefail

# RustOwl Development Checks and Fixes Script
# This script runs local development checks and can optionally fix issues

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

RUST_MIN_VERSION="1.87"
AUTO_FIX=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

show_help() {
    echo "RustOwl Development Checks and Fixes"
    echo ""
    echo "USAGE:"
    echo "    $0 [OPTIONS]"
    echo ""
    echo "OPTIONS:"
    echo "    -h, --help     Show this help message"
    echo "    -f, --fix      Automatically fix issues where possible"
    echo "    --check-only   Only run checks, don't fix anything (default)"
    echo ""
    echo "CHECKS PERFORMED:"
    echo "    - Rust toolchain version (minimum $RUST_MIN_VERSION)"
    echo "    - Code formatting (rustfmt)"
    echo "    - Linting (clippy)"
    echo "    - Build test"
    echo "    - VS Code extension checks (if yarn is available)"
    echo ""
    echo "FIXES APPLIED (with --fix):"
    echo "    - Format code with rustfmt"
    echo "    - Apply clippy suggestions where possible"
    echo "    - Format VS Code extension code"
    echo ""
    echo "EXAMPLES:"
    echo "    $0                 # Run checks only"
    echo "    $0 --fix           # Run checks and fix issues"
    echo "    $0 --check-only    # Explicitly run checks only"
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

check_rust_version() {
    log_info "Checking Rust version..."
    
    if ! command -v rustc &> /dev/null; then
        log_error "rustc not found. Please install Rust."
        return 1
    fi
    
    local rust_version
    rust_version=$(rustc --version | grep -oE '[0-9]+\.[0-9]+' | head -1)
    
    if [ -z "$rust_version" ]; then
        log_error "Could not determine Rust version"
        return 1
    fi
    
    # Compare versions (basic comparison for major.minor)
    local min_major min_minor cur_major cur_minor
    min_major=$(echo "$RUST_MIN_VERSION" | cut -d. -f1)
    min_minor=$(echo "$RUST_MIN_VERSION" | cut -d. -f2)
    cur_major=$(echo "$rust_version" | cut -d. -f1)
    cur_minor=$(echo "$rust_version" | cut -d. -f2)
    
    if [ "$cur_major" -lt "$min_major" ] || 
       ([ "$cur_major" -eq "$min_major" ] && [ "$cur_minor" -lt "$min_minor" ]); then
        log_error "Rust version $rust_version is below minimum required version $RUST_MIN_VERSION"
        return 1
    fi
    
    log_success "Rust version $rust_version >= $RUST_MIN_VERSION"
}

check_formatting() {
    log_info "Checking code formatting..."
    
    if $AUTO_FIX; then
        log_info "Applying code formatting..."
        if cargo fmt; then
            log_success "Code formatted successfully"
        else
            log_error "Failed to format code"
            return 1
        fi
    else
        if cargo fmt --check; then
            log_success "Code is properly formatted"
        else
            log_error "Code formatting issues found. Run with --fix to auto-format."
            return 1
        fi
    fi
}

check_clippy() {
    log_info "Running clippy lints..."
    
    if $AUTO_FIX; then
        log_info "Applying clippy fixes where possible..."
        # First try to fix what we can
        if cargo clippy --fix --allow-dirty --allow-staged 2>/dev/null || true; then
            log_info "Applied some clippy fixes"
        fi
        # Then check for remaining issues
        if cargo clippy --all-targets --all-features -- -D warnings; then
            log_success "All clippy checks passed"
        else
            log_warning "Some clippy issues remain that couldn't be auto-fixed"
            return 1
        fi
    else
        if cargo clippy --all-targets --all-features -- -D warnings; then
            log_success "All clippy checks passed"
        else
            log_error "Clippy found issues. Run with --fix to apply automatic fixes."
            return 1
        fi
    fi
}

check_build() {
    log_info "Testing build..."
    
    if cargo build --release; then
        log_success "Build successful"
    else
        log_error "Build failed"
        return 1
    fi
}

check_tests() {
    log_info "Checking for unit tests..."
    
    # Check if there are actual unit tests (not doc tests)
    local unit_test_output
    unit_test_output=$(cargo test --lib --bins 2>&1)
    
    # Count only unit tests, not doc tests
    local unit_test_count
    unit_test_count=$(echo "$unit_test_output" | grep -E "running [0-9]+ tests" | awk '{sum += $2} END {print sum+0}')
    
    if [ "$unit_test_count" -eq 0 ]; then
        log_info "No unit tests found (this is expected for RustOwl)"
        return 0
    else
        log_info "Running $unit_test_count unit tests..."
        if cargo test --lib --bins; then
            log_success "All unit tests passed"
        else
            log_error "Some unit tests failed"
            return 1
        fi
    fi
}

check_vscode_extension() {
    if [ ! -d "vscode" ]; then
        log_info "VS Code extension directory not found, skipping"
        return 0
    fi
    
    log_info "Checking VS Code extension..."
    
    if ! command -v yarn &> /dev/null; then
        log_warning "yarn not found, skipping VS Code extension checks"
        return 0
    fi
    
    cd vscode
    
    # Install dependencies if needed
    if [ ! -d "node_modules" ]; then
        log_info "Installing VS Code extension dependencies..."
        yarn install --frozen-lockfile
    fi
    
    if $AUTO_FIX; then
        log_info "Formatting VS Code extension code..."
        if yarn prettier --write src; then
            log_success "VS Code extension code formatted"
        else
            log_warning "Failed to format VS Code extension code"
        fi
    else
        if yarn prettier --check src; then
            log_success "VS Code extension code is properly formatted"
        else
            log_error "VS Code extension formatting issues found. Run with --fix to auto-format."
            cd "$REPO_ROOT"
            return 1
        fi
    fi
    
    # Type checking and linting
    if yarn lint && yarn check-types; then
        log_success "VS Code extension checks passed"
    else
        log_error "VS Code extension checks failed"
        cd "$REPO_ROOT"
        return 1
    fi
    
    cd "$REPO_ROOT"
}

main() {
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -f|--fix)
                AUTO_FIX=true
                shift
                ;;
            --check-only)
                AUTO_FIX=false
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    log_info "Starting development checks..."
    if $AUTO_FIX; then
        log_info "Auto-fix mode enabled"
    else
        log_info "Check-only mode (use --fix to enable auto-fixes)"
    fi
    echo ""
    
    local failed_checks=0
    
    # Run all checks
    check_rust_version || ((failed_checks++))
    check_formatting || ((failed_checks++))
    check_clippy || ((failed_checks++))
    check_build || ((failed_checks++))
    check_tests || ((failed_checks++))
    check_vscode_extension || ((failed_checks++))
    
    echo ""
    if [ $failed_checks -eq 0 ]; then
        log_success "All development checks passed! ✅"
        exit 0
    else
        log_error "$failed_checks check(s) failed"
        if ! $AUTO_FIX; then
            log_info "Try running with --fix to automatically resolve some issues"
        fi
        exit 1
    fi
}

main "$@"
