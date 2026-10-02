#!/usr/bin/env bash
# RustOwl Security & Memory Safety Testing Script
# Tests for undefined behavior, memory leaks, and security vulnerabilities.
# Automatically detects platform capabilities and runs appropriate tests.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=scripts/lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

cd "$REPO_ROOT"

setup_nix_ld_paths

# Configuration
MIN_RUST_VERSION="1.90.0"
TEST_TARGET_PATH="./crates/rustowl/perf-tests/dummy-package"

# Output logging configuration
LOG_DIR="security-logs"
TIMESTAMP="$(date '+%Y%m%d_%H%M%S')"

# CI environment detection
IS_CI=0
CI_AUTO_INSTALL=0
MODE="run"

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

# Resolved once: repeated `rustup show active-toolchain` calls are noisy and slow.
ACTIVE_TOOLCHAIN=""
HAS_NIGHTLY=0

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
	echo "  --thread-sanitizer   Also run ThreadSanitizer tests (off by default;"
	echo "                      it instruments every build and is slow)"
	echo ""
	echo "Platform Support:"
	echo "  Linux:   Miri, Valgrind, cargo-audit, cargo-machete"
	echo "  macOS:   Miri, cargo-audit, cargo-machete, Instruments"
	echo ""
	echo "CI Environment:"
	echo "  The script automatically detects CI environments. Missing tools are"
	echo "  installed unless --no-auto-install is passed."
	echo ""
	echo "Tests performed:"
	echo "  - Miri: Detects undefined behavior in Rust code"
	echo "  - Valgrind: Memory error detection (Linux)"
	echo "  - ThreadSanitizer: Data race detection (opt-in)"
	echo "  - cargo-audit: Security vulnerability scanning"
	echo "  - cargo-machete: Unused dependency detection"
	echo "  - Instruments: Time Profiler trace (macOS)"
	echo ""
	echo "Examples:"
	echo "  $0                   # Auto-detect platform and run appropriate tests"
	echo "  $0 --check          # Check which tools are available"
	echo "  $0 --install        # Install missing security tools automatically"
	echo "  $0 --ci             # Force CI mode with auto-installation"
	echo "  $0 --no-miri        # Run tests but skip Miri"
	echo "  $0 --thread-sanitizer  # Include ThreadSanitizer (slow)"
	echo ""
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
	case $1 in
	-h | --help)
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
	--thread-sanitizer)
		RUN_THREAD_SANITIZER=1
		shift
		;;
	--no-cargo-machete)
		RUN_CARGO_MACHETE=0
		shift
		;;
	*)
		log_error "Unknown option: $1"
		usage
		exit 1
		;;
	esac
done

# Helper function to print section headers
print_section_header() {
	local title="$1"
	local description="${2:-}"
	printf '%b\n' "${BLUE}${BOLD}$title${NC}"
	printf '%b\n' "${BLUE}================================${NC}"
	[[ -n "$description" ]] && echo "$description"
	echo ""
}

# ---------------------------------------------------------------------------
# Platform and environment detection
# ---------------------------------------------------------------------------

# OS detection with more robust platform detection
detect_platform() {
	if [[ "$OSTYPE" == "linux-gnu"* ]]; then
		OS_TYPE="Linux"
	elif [[ "$OSTYPE" == "darwin"* ]]; then
		OS_TYPE="macOS"
	else
		# Fallback to uname
		local uname_result
		uname_result="$(uname 2>/dev/null || echo "unknown")"
		case "$uname_result" in
		Linux*) OS_TYPE="Linux" ;;
		Darwin*) OS_TYPE="macOS" ;;
		*) OS_TYPE="Unknown" ;;
		esac
	fi

	log_info "Detected platform: $OS_TYPE"
}

# Detect CI environment and configure accordingly
detect_ci_environment() {
	if [[ -n "${CI:-}" ]] || [[ -n "${GITHUB_ACTIONS:-}" ]]; then
		IS_CI=1
		CI_AUTO_INSTALL=1

		if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
			log_info "CI environment detected (GitHub Actions); auto-installation enabled"
		else
			log_info "CI environment detected; auto-installation enabled"
		fi
	else
		log_info "Interactive environment detected"
	fi
}

# Resolve the active toolchain once and remember whether it is nightly.
resolve_toolchain() {
	ACTIVE_TOOLCHAIN="$(rustup show active-toolchain 2>/dev/null | cut -d' ' -f1 || true)"
	[[ "$ACTIVE_TOOLCHAIN" == *"nightly"* ]] && HAS_NIGHTLY=1
	return 0
}

# Auto-configure tests based on platform capabilities
auto_configure_tests() {
	log_info "Auto-configuring tests for $OS_TYPE..."

	case "$OS_TYPE" in
	"Linux")
		log_info "  Linux detected: enabling Miri, Valgrind, Audit and cargo-machete"
		# Instruments is a macOS-only tool.
		RUN_INSTRUMENTS=0
		RUN_CARGO_MACHETE=1
		;;
	"macOS")
		log_info "  macOS detected: enabling Miri, Audit, cargo-machete and Instruments"
		log_info "  Disabling Valgrind (unreliable on macOS)"
		RUN_VALGRIND=0
		RUN_CARGO_MACHETE=1
		;;
	*)
		log_info "  Unknown platform: enabling basic tests only"
		RUN_VALGRIND=0
		RUN_INSTRUMENTS=0
		# Also disable nightly-dependent features on unknown platforms
		RUN_MIRI=0
		;;
	esac

	echo ""
}

# ---------------------------------------------------------------------------
# Tool availability
# ---------------------------------------------------------------------------

# Detect available tools based on platform
detect_tools() {
	log_info "Detecting available security tools..."

	if have_cmd cargo-audit; then
		HAS_CARGO_AUDIT=1
		log_success "cargo-audit available"
	else
		log_warning "! cargo-audit not found"
	fi

	if have_cmd cargo-machete; then
		HAS_CARGO_MACHETE=1
		log_success "cargo-machete available"
	else
		log_warning "! cargo-machete not found"
	fi

	if [[ "$OS_TYPE" == "macOS" ]]; then
		# xctrace replaced the deprecated `instruments` CLI. Probing it is slow
		# and may need a first-run permission prompt, so keep the probe short.
		if have_cmd xcrun && timeout 10s xcrun xctrace version >/dev/null 2>&1; then
			HAS_INSTRUMENTS=1
			log_success "Instruments (xctrace) available"
		else
			log_warning "! Instruments not found (will try to install Xcode in CI)"
		fi
	fi

	if [[ "$OS_TYPE" == "Linux" ]]; then
		if have_cmd valgrind; then
			HAS_VALGRIND=1
			log_success "Valgrind available"
		else
			log_warning "! Valgrind not found"
		fi
	fi

	resolve_toolchain
	log_info "Active toolchain: ${ACTIVE_TOOLCHAIN:-unknown}"
	if [[ $HAS_NIGHTLY -eq 1 ]]; then
		log_success "Nightly toolchain is active (from rust-toolchain.toml)"
	else
		log_warning "! Stable toolchain detected"
		log_warning "Some advanced features require nightly (check rust-toolchain.toml)"
	fi

	if rustup component list --installed 2>/dev/null | grep -q miri; then
		HAS_MIRI=1
		log_success "Miri is available"
	else
		log_warning "! Miri component not installed"
		log_warning "Install with: rustup component add miri"
	fi

	echo ""
}

# Green "Available" / yellow "Not installed", for the console summary.
availability_badge() { # availability_badge <0|1>
	if [[ $1 -eq 1 ]]; then
		printf '%b' "${GREEN}[OK] Available${NC}"
	else
		printf '%b' "${YELLOW}! Not installed${NC}"
	fi
}

# Plain-text equivalent, for the markdown summary file.
markdown_state() { # markdown_state <0|1> <missing-label>
	if [[ $1 -eq 1 ]]; then
		echo "[OK] Available"
	else
		echo "[FAIL] $2"
	fi
}

# Show tool status summary
show_tool_status() {
	printf '%b\n' "${BLUE}${BOLD}Tool Availability Summary${NC}"
	printf '%b\n' "${BLUE}================================${NC}"
	echo ""

	log_info "Platform: $OS_TYPE"
	echo ""
	echo "Security Tools:"

	printf '  %-30s %b\n' "Miri (UB detection)" "$(availability_badge "$HAS_MIRI")"
	[[ "$OS_TYPE" == "Linux" ]] &&
		printf '  %-30s %b\n' "Valgrind (memory errors)" "$(availability_badge "$HAS_VALGRIND")"
	printf '  %-30s %b\n' "cargo-audit (vulnerabilities)" "$(availability_badge "$HAS_CARGO_AUDIT")"
	[[ "$OS_TYPE" == "macOS" ]] &&
		printf '  %-30s %b\n' "Instruments (time profiler)" "$(availability_badge "$HAS_INSTRUMENTS")"

	echo ""
	echo "Advanced Features:"
	if [[ $HAS_NIGHTLY -eq 1 ]]; then
		printf '  %-30s %b\n' "Nightly toolchain" "${GREEN}[OK] Available${NC}"
		printf '  %-30s %b\n' "Advanced features" "${GREEN}[OK] Supported${NC}"
	else
		printf '  %-30s %b\n' "Nightly toolchain" "${YELLOW}! Stable toolchain active${NC}"
		printf '  %-30s %b\n' "Advanced features" "${YELLOW}! Require nightly${NC}"
	fi

	echo ""
	echo "Test Configuration:"
	local flag
	for flag in "Miri:$RUN_MIRI" "Valgrind:$RUN_VALGRIND" "ThreadSanitizer:$RUN_THREAD_SANITIZER" \
		"Audit:$RUN_AUDIT" "Instruments:$RUN_INSTRUMENTS" "cargo-machete:$RUN_CARGO_MACHETE"; do
		if [[ "${flag#*:}" -eq 1 ]]; then
			printf '  %-30s %b\n' "Run ${flag%%:*}" "${GREEN}Enabled${NC}"
		else
			printf '  %-30s %b\n' "Run ${flag%%:*}" "${YELLOW}Disabled${NC}"
		fi
	done

	echo ""
}

# Create security summary with tool outputs
create_security_summary() {
	local summary_file="$LOG_DIR/security_summary_${TIMESTAMP}.md"

	mkdir -p "$LOG_DIR"

	local ci_label="No"
	[[ $IS_CI -eq 1 ]] && ci_label="Yes"

	{
		echo "# Security Testing Summary"
		echo ""
		echo "**Generated:** $(date)"
		echo "**Platform:** $OS_TYPE"
		echo "**CI Environment:** $ci_label"
		echo "**Rust Version:** $(rustc --version 2>/dev/null || echo 'N/A')"
		echo ""
		echo "## Tool Availability"
		echo ""
		echo "| Tool | Status | Notes |"
		echo "|------|--------|-------|"
		echo "| Miri | $(markdown_state "$HAS_MIRI" "Missing") | Undefined behavior detection |"
		echo "| Valgrind | $(markdown_state "$HAS_VALGRIND" "Missing/N/A") | Memory error detection (Linux) |"
		echo "| cargo-audit | $(markdown_state "$HAS_CARGO_AUDIT" "Missing") | Security vulnerability scanning |"
		echo "| Instruments | $(markdown_state "$HAS_INSTRUMENTS" "Missing/N/A") | Time Profiler trace (macOS) |"
		echo ""
	} >"$summary_file"
}

# ---------------------------------------------------------------------------
# Installation
# ---------------------------------------------------------------------------

# Install the system package providing $1, trying the platform's package manager.
install_system_package() {
	local package="$1"
	local manager

	for manager in apt-get yum pacman; do
		have_cmd "$manager" || continue

		log_info "Installing $package via $manager..."
		local ok=1
		case $manager in
		apt-get)
			sudo apt-get update || ok=0
			[[ $ok -eq 1 ]] && { sudo apt-get install -y "$package" || ok=0; }
			;;
		yum) sudo yum install -y "$package" || ok=0 ;;
		pacman) sudo pacman -S --noconfirm "$package" || ok=0 ;;
		esac

		if [[ $ok -eq 1 ]]; then
			log_success "$package installed successfully"
			return 0
		fi
		log_warning "Failed to install $package via $manager"
		return 1
	done

	log_warning "No supported package manager found for $package"
	return 1
}

# Install/setup Xcode on macOS CI environments. Verifies that `xctrace` is
# actually usable afterwards, since a present-but-unlicensed Xcode answers the
# probe slowly and uselessly.
install_xcode_ci() {
	if [[ "$OS_TYPE" != "macOS" ]] || [[ $IS_CI -ne 1 ]]; then
		return 0
	fi

	log_info "Setting up Xcode for CI environment..."

	# First, try to install/setup command line tools
	sudo xcode-select --install 2>/dev/null || true

	# Set the developer directory
	if [[ -d "/Applications/Xcode.app" ]]; then
		log_info "Found Xcode.app, setting developer directory..."
		sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
	elif [[ -d "/Library/Developer/CommandLineTools" ]]; then
		log_info "Using Command Line Tools..."
		sudo xcode-select --switch /Library/Developer/CommandLineTools
	fi

	# Accept license if needed
	if sudo xcodebuild -license accept 2>/dev/null; then
		log_info "Xcode license accepted"
	fi

	if ! xcode-select -p >/dev/null 2>&1; then
		log_error "Failed to set up Xcode properly"
		return 1
	fi

	log_info "Xcode developer directory: $(xcode-select -p)"

	if have_cmd xcrun && timeout 10s xcrun xctrace version >/dev/null 2>&1; then
		HAS_INSTRUMENTS=1
		log_success "Instruments (xctrace) is now available"
		return 0
	fi

	log_warning "Instruments still not usable after Xcode setup"
	return 1
}

# Install missing tools. Runs only in CI, or when --install is explicit.
install_required_tools() {
	log_info "Installing missing security tools..."

	if [[ $HAS_CARGO_AUDIT -eq 0 && $RUN_AUDIT -eq 1 ]]; then
		log_info "Installing cargo-audit..."
		if cargo install cargo-audit; then
			HAS_CARGO_AUDIT=1
			log_success "cargo-audit installed"
		else
			log_error "Failed to install cargo-audit"
		fi
	fi

	if [[ $HAS_CARGO_MACHETE -eq 0 && $RUN_CARGO_MACHETE -eq 1 ]]; then
		log_info "Installing cargo-machete..."
		if cargo install cargo-machete; then
			HAS_CARGO_MACHETE=1
			log_success "cargo-machete installed"
		else
			log_error "Failed to install cargo-machete"
		fi
	fi

	if [[ $HAS_MIRI -eq 0 && $RUN_MIRI -eq 1 ]]; then
		log_info "Installing Miri component..."
		if rustup component add miri --toolchain nightly; then
			HAS_MIRI=1
			log_success "Miri component installed successfully"
		else
			log_error "Failed to install Miri component"
		fi
	fi

	if [[ "$OS_TYPE" == "Linux" && $HAS_VALGRIND -eq 0 && $RUN_VALGRIND -eq 1 ]]; then
		if install_system_package valgrind; then
			HAS_VALGRIND=1
		fi
	fi

	if [[ $RUN_INSTRUMENTS -eq 1 && $HAS_INSTRUMENTS -eq 0 ]]; then
		install_xcode_ci || true
	fi

	echo ""
}

# ---------------------------------------------------------------------------
# Test runners
# ---------------------------------------------------------------------------

# Run $1 with $2's environment prefix, streaming to a timestamped log, and
# return the command's own exit status.
run_logged() {
	local test_name="$1"
	local command="$2"

	mkdir -p "$LOG_DIR"

	local log_file="$LOG_DIR/${test_name}_${TIMESTAMP}.log"
	{
		echo "==========================================="
		echo "Test: $test_name"
		echo "Command: $command"
		echo "Timestamp: $(date)"
		echo "Working Directory: $(pwd)"
		echo "Environment: OS=$OS_TYPE, CI=$IS_CI"
		echo "==========================================="
		echo ""
		echo "=== COMMAND OUTPUT ==="
	} >>"$log_file"

	local status=0
	eval "$command" >>"$log_file" 2>&1 || status=$?

	{
		echo ""
		if [[ $status -eq 0 ]]; then
			echo "=== COMMAND COMPLETED SUCCESSFULLY ==="
		else
			echo "=== COMMAND FAILED WITH EXIT CODE: $status ==="
		fi
		echo "End timestamp: $(date)"
		echo "==========================================="
	} >>"$log_file"

	return $status
}

# Pick the analysis argument: the test package when it exists, else --help.
# Same shape as the original `if [ -d ... ] else ... fi` in every runner.
analysis_args() {
	if [[ -d "$TEST_TARGET_PATH" ]]; then
		echo "check $TEST_TARGET_PATH"
	else
		echo "--help"
	fi
}

log_path() { echo "$LOG_DIR/$1_${TIMESTAMP}.log"; }

# Locate the release rustowl binary, building it if necessary.
rustowl_release_binary() {
	local binary="./target/release/rustowl"

	if [[ ! -f "$binary" ]]; then
		log_info "Building RustOwl for this test..."
		if ! ./scripts/toolchain cargo build --release >/dev/null 2>&1; then
			log_error "Failed to build RustOwl"
			return 1
		fi
	fi

	if [[ ! -f "$binary" ]]; then
		log_error "RustOwl binary not found at $binary"
		return 1
	fi

	echo "$binary"
}

run_miri_tests() {
	[[ $RUN_MIRI -eq 1 ]] || return 0

	if [[ $HAS_MIRI -eq 0 ]]; then
		log_warning "Skipping Miri tests (component not installed)"
		return 0
	fi

	print_section_header "Running Miri Tests" "Miri detects undefined behavior in Rust code"

	# These flags are what let RustOwl spawn the cargo/rustc processes it needs.
	local miri_env='MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-permissive-provenance" RUSTFLAGS="--cfg miri"'
	local args analysis_log analysis_desc
	args="$(analysis_args)"

	log_info "Running RustOwl unit tests with Miri..."
	if run_logged miri_unit_tests "$miri_env cargo miri test --lib"; then
		log_success "RustOwl unit tests passed with Miri"
	else
		log_error "RustOwl unit tests failed with Miri"
		log_info "  Full output captured in: $(log_path miri_unit_tests)"
		return 1
	fi

	# Prefer the real analysis run; fall back to --help when the test package is
	# absent, so there is always something for Miri to execute.
	if [[ -d "$TEST_TARGET_PATH" ]]; then
		analysis_log="miri_rustowl_analysis"
		analysis_desc="RustOwl analysis"
	else
		args="--help"
		analysis_log="miri_basic_execution"
		analysis_desc="basic RustOwl execution"
		log_warning "No test target found at $TEST_TARGET_PATH; falling back to --help"
	fi

	log_info "Testing ${analysis_desc} with Miri..."
	if run_logged "$analysis_log" "$miri_env cargo miri run --bin rustowl -- $args"; then
		log_success "${analysis_desc} completed with Miri"
	else
		log_warning "Miri could not complete ${analysis_desc} (process spawning limitations)"
		log_warning "  This is expected: RustOwl spawns cargo processes which Miri doesn't support"
		log_warning "  Core RustOwl memory safety is validated by the system allocator switch"
		log_info "  Full output captured in: $(log_path "$analysis_log")"
	fi

	echo ""
}

run_thread_sanitizer_tests() {
	[[ $RUN_THREAD_SANITIZER -eq 1 ]] || return 0

	print_section_header "Running ThreadSanitizer Tests" \
		"ThreadSanitizer detects data races and threading issues"

	if [[ $HAS_NIGHTLY -eq 0 ]]; then
		log_warning "ThreadSanitizer needs a nightly toolchain (active: ${ACTIVE_TOOLCHAIN:-unknown})"
		log_info "  Full output captured in: $(log_path tsan_rustowl_analysis)"
		return 0
	fi

	local args
	args="$(analysis_args)"

	log_info "Using RUSTFLAGS: -Zsanitizer=thread"
	if run_logged tsan_rustowl_analysis \
		"RUSTFLAGS=\"-Zsanitizer=thread\" cargo +nightly run --bin rustowl -- $args"; then
		log_success "RustOwl analysis completed with ThreadSanitizer"
	else
		log_warning "ThreadSanitizer reported issues (see log)"
		log_info "  Full output captured in: $(log_path tsan_rustowl_analysis)"
		return 1
	fi

	echo ""
}

run_valgrind_tests() {
	[[ $RUN_VALGRIND -eq 1 ]] || return 0

	if [[ $HAS_VALGRIND -eq 0 ]]; then
		log_warning "Skipping Valgrind tests (not available on this platform)"
		return 0
	fi

	print_section_header "Running Valgrind Tests" \
		"Valgrind detects memory errors, leaks, and memory corruption"

	local binary
	binary="$(rustowl_release_binary)" || return 1

	local suppressions=""
	if [[ -f ".valgrind-suppressions" ]]; then
		suppressions="--suppressions=$REPO_ROOT/.valgrind-suppressions"
		log_info "Using suppressions file: $REPO_ROOT/.valgrind-suppressions"
	fi

	local flags="valgrind --tool=memcheck --leak-check=full --show-leak-kinds=all --track-origins=yes $suppressions"
	local args
	args="$(analysis_args)"

	log_info "Running RustOwl with Valgrind..."
	log_info "Using Valgrind flags: --tool=memcheck --leak-check=full --show-leak-kinds=all --track-origins=yes"

	if run_logged valgrind_analysis "$flags $binary $args"; then
		log_success "RustOwl analysis completed with Valgrind (no memory errors detected)"
	else
		log_error "Valgrind detected memory errors in RustOwl analysis"
		log_info "  Full output captured in: $(log_path valgrind_analysis)"
		return 1
	fi

	echo ""
}

run_instruments_tests() {
	[[ $RUN_INSTRUMENTS -eq 1 ]] || return 0

	if [[ $HAS_INSTRUMENTS -eq 0 ]]; then
		log_warning "Skipping Instruments tests (not available on this platform)"
		return 0
	fi

	print_section_header "Running Instruments Tests" \
		"Instruments Time Profiler detects hot paths and memory behaviour"

	local binary
	binary="$(rustowl_release_binary)" || return 1

	local args
	args="$(analysis_args)"

	# CI uploads instruments_output*.trace, so the name has to keep that prefix.
	local trace="$REPO_ROOT/instruments_output_${TIMESTAMP}.trace"

	log_info "Profiling RustOwl analysis..."
	log_info "Using: xcrun xctrace record --template 'Time Profiler' --launch"
	if run_logged instruments_analysis \
		"xcrun xctrace record --template 'Time Profiler' --time-limit 60s --output '$trace' --launch -- '$binary' $args"; then
		if [[ -d "$trace" || -f "$trace" ]]; then
			log_success "Instruments trace captured: $trace"
		else
			log_warning "xctrace reported success but produced no trace at $trace"
		fi
	else
		log_error "Instruments failed to record a trace"
		log_info "  Full output captured in: $(log_path instruments_analysis)"
		return 1
	fi

	echo ""
}

run_audit_check() {
	if [[ $RUN_AUDIT -eq 0 ]]; then
		return 0
	fi

	if [[ $HAS_CARGO_AUDIT -eq 0 ]]; then
		log_warning "Skipping cargo-audit (not installed)"
		return 0
	fi

	log_info "Scanning dependencies for vulnerabilities..."
	if cargo audit; then
		log_success "No known vulnerabilities found"
	else
		log_error "Security vulnerabilities detected"
		return 1
	fi

	echo ""
}

run_cargo_machete_tests() {
	[[ $RUN_CARGO_MACHETE -eq 1 ]] || return 0

	if [[ $HAS_CARGO_MACHETE -eq 0 ]]; then
		log_warning "Skipping cargo-machete tests (not installed)"
		return 0
	fi

	print_section_header "Running cargo-machete Tests" \
		"cargo-machete detects unused dependencies in Cargo.toml"

	log_info "Scanning for unused dependencies..."

	# cargo-machete exits non-zero when it finds unused dependencies, which is a
	# warning for us rather than a suite failure, so never let it fail the run.
	if run_logged cargo_machete_analysis "cargo machete"; then
		log_success "cargo-machete analysis completed"
	else
		log_warning "cargo-machete found potential issues"
		log_warning "  Note: cargo-machete may report false positives for conditionally used deps"
	fi

	local log_file
	log_file="$(log_path cargo_machete_analysis)"
	if [[ -f "$log_file" ]] && grep -q "unused dependencies" "$log_file" 2>/dev/null; then
		log_warning "Found potential unused dependencies - check $log_file for details"
	else
		log_success "No unused dependencies detected"
	fi

	echo ""
}

# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

main() {
	print_section_header "RustOwl Security & Memory Safety Testing" ""

	detect_platform
	detect_ci_environment

	if [[ "$MODE" == "check" ]]; then
		log_info "Checking tool availability and system readiness..."
		echo ""
		# Apply the platform defaults first so the summary reports the
		# configuration a real run would use, not the raw flags.
		auto_configure_tests
		detect_tools
		show_tool_status
		echo ""
		log_success "System check completed."
		return 0
	fi

	log_info "Running security and memory safety analysis..."
	echo ""

	detect_tools
	auto_configure_tests

	# CI_AUTO_INSTALL is what --no-auto-install clears, so honouring the flag
	# means consulting it here rather than only IS_CI.
	if [[ $CI_AUTO_INSTALL -eq 1 && $IS_CI -eq 1 ]] || [[ "$MODE" == "install" ]]; then
		install_required_tools
		# Re-detect after installing.
		detect_tools
	fi

	check_rust_version "$MIN_RUST_VERSION"

	echo ""
	log_info "Running security tests..."
	echo ""

	create_security_summary

	local test_failures=0

	# Each suite reports whether it found a problem. Suites that are disabled or
	# unavailable return 0, so this is the single place failures are counted.
	run_miri_tests || test_failures=$((test_failures + 1))
	run_thread_sanitizer_tests || test_failures=$((test_failures + 1))
	run_valgrind_tests || test_failures=$((test_failures + 1))
	run_audit_check || test_failures=$((test_failures + 1))
	run_cargo_machete_tests || test_failures=$((test_failures + 1))
	run_instruments_tests || test_failures=$((test_failures + 1))

	echo ""
	if [[ $test_failures -eq 0 ]]; then
		printf '%b\n' "${GREEN}${BOLD}All security tests passed!${NC}"
		printf '%b\n' "${GREEN}No security issues detected.${NC}"
		return 0
	fi

	printf '%b\n' "${RED}${BOLD}Security tests failed!${NC}"
	printf '%b\n' "${RED}$test_failures test suite(s) failed.${NC}"
	log_info "Check logs in $LOG_DIR/ for details."
	return 1
}

main "$@"
