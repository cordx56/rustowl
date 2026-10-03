#!/usr/bin/env bash

set -euo pipefail

# RustOwl Development Checks and Fixes Script
# This script runs local development checks and can optionally fix issues

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=scripts/lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

cd "$REPO_ROOT"

setup_nix_ld_paths

RUST_MIN_VERSION="1.87"
AUTO_FIX=false

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
	echo "    - Linting (clippy, all targets and features)"
	echo "    - Release build"
	echo "    - Unit tests (library and binaries)"
	echo "    - VS Code extension checks (skipped if pnpm is unavailable)"
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

# Thin wrapper so every check in the loop below has the same signature.
check_toolchain_version() {
	check_rust_version "$RUST_MIN_VERSION"
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
		return 0
	fi

	if cargo fmt --check; then
		log_success "Code is properly formatted"
	else
		log_error "Code formatting issues found. Run with --fix to auto-format."
		return 1
	fi
}

check_clippy() {
	log_info "Running clippy lints..."

	if $AUTO_FIX; then
		log_info "Applying clippy fixes where possible..."
		# Best effort only: most clippy suggestions are not machine-applicable,
		# and a failure here must not skip the real lint run below.
		cargo clippy --fix --allow-dirty --allow-staged 2>/dev/null ||
			log_info "No automatically applicable clippy fixes"
	fi

	if cargo clippy --all-targets --all-features -- -D warnings; then
		log_success "All clippy checks passed"
		return 0
	fi

	if $AUTO_FIX; then
		log_warning "Some clippy issues remain that couldn't be auto-fixed"
	else
		log_error "Clippy found issues. Run with --fix to apply automatic fixes."
	fi
	return 1
}

check_build() {
	log_info "Testing build..."

	if ./scripts/toolchain cargo build --release; then
		log_success "Build successful"
	else
		log_error "Build failed"
		return 1
	fi
}

# One run, not two: the test count comes from the same output, and a plain
# assignment here would let `set -e` abort the script on the first failure.
check_tests() {
	log_info "Checking for unit tests..."

	local output status count

	if output="$(cargo test --lib --bins 2>&1)"; then
		status=0
	else
		status=$?
	fi

	# `running N tests` is printed once per test binary; doc tests are excluded
	# because we only pass --lib --bins.
	count="$(printf '%s\n' "$output" | awk '/^running [0-9]+ tests?$/ { sum += $2 } END { print sum + 0 }')"

	# The output of the single run stands in for the old second run.
	printf '%s\n' "$output"

	# Zero tests is only a pass when cargo itself succeeded. A test-only
	# compile error also yields no "running N tests" lines, and reporting that
	# as "no unit tests found" would hide a broken build behind a green check.
	if [ "$count" -eq 0 ] && [ "$status" -eq 0 ]; then
		log_info "No unit tests found (this is expected for RustOwl)"
		return 0
	fi

	if [ "$status" -eq 0 ]; then
		log_success "All $count unit tests passed"
		return 0
	fi

	log_error "Some unit tests failed"
	return 1
}

# Run entirely in a subshell so the `cd vscode` cannot leak into the next check
# and no manual `cd "$REPO_ROOT"` restore is needed on each error path.
check_vscode_extension() {
	if [ ! -d "vscode" ]; then
		log_info "VS Code extension directory not found, skipping"
		return 0
	fi

	log_info "Checking VS Code extension..."

	if ! have_cmd pnpm; then
		log_warning "pnpm not found, skipping VS Code extension checks"
		return 0
	fi

	(
		cd vscode

		if [ ! -d "node_modules" ]; then
			log_info "Installing VS Code extension dependencies..."
			pnpm install --frozen-lockfile
		fi

		# Use the extension's own scripts (see vscode/package.json) so the scope
		# we check is the scope the project formats.
		if $AUTO_FIX; then
			log_info "Formatting VS Code extension code..."
			if pnpm run fmt; then
				log_success "VS Code extension code formatted"
			else
				log_warning "Failed to format VS Code extension code"
			fi
		elif pnpm exec prettier --check .; then
			log_success "VS Code extension code is properly formatted"
		else
			log_error "VS Code extension formatting issues found. Run with --fix to auto-format."
			exit 1
		fi

		if pnpm run lint && pnpm run check-types; then
			log_success "VS Code extension checks passed"
		else
			log_error "VS Code extension checks failed"
			exit 1
		fi
	)
}

main() {
	while [[ $# -gt 0 ]]; do
		case $1 in
		-h | --help)
			show_help
			exit 0
			;;
		-f | --fix)
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

	# Written out rather than looped: a loop would hide every function here from
	# the never-invoked-function lint. And `((failed_checks++))` returns the
	# pre-increment value, so the first failure would trip `set -e`.
	check_toolchain_version || failed_checks=$((failed_checks + 1))
	check_formatting || failed_checks=$((failed_checks + 1))
	check_clippy || failed_checks=$((failed_checks + 1))
	check_build || failed_checks=$((failed_checks + 1))
	check_tests || failed_checks=$((failed_checks + 1))
	check_vscode_extension || failed_checks=$((failed_checks + 1))

	echo ""
	if [ "$failed_checks" -eq 0 ]; then
		log_success "All development checks passed! ✅"
		exit 0
	fi

	log_error "$failed_checks check(s) failed"
	if ! $AUTO_FIX; then
		log_info "Try running with --fix to automatically resolve some issues"
	fi
	exit 1
}

main "$@"
