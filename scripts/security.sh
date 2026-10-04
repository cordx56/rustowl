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
CI_PROVIDER="generic CI"
MODE="run"
# Sticky opt-out set only by --no-auto-install. Nothing else writes it, so CI
# detection cannot re-enable what the caller asked to disable.
NO_AUTO_INSTALL=0
# Sticky opt-out set only by --no-cargo-shear. auto_configure_tests() enables
# cargo-shear on Linux and macOS and runs after argument parsing, so clearing
# RUN_CARGO_SHEAR alone was silently undone.
NO_CARGO_SHEAR=0

# Test flags (can be overridden via command line options)
RUN_MIRI=1
RUN_VALGRIND=1
RUN_CARGO_DENY=1
RUN_INSTRUMENTS=1
RUN_THREAD_SANITIZER=0
RUN_CARGO_SHEAR=0

# Exit code valgrind substitutes when it counts a memory error. Lets the suite
# tell "valgrind found a problem" apart from "RustOwl exited non-zero".
VALGRIND_ERROR_EXIT=42

# Tool availability detection
HAS_MIRI=0
HAS_VALGRIND=0
HAS_CARGO_DENY=0
HAS_INSTRUMENTS=0
HAS_CARGO_SHEAR=0

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
	echo "  --no-audit           Skip the cargo-deny vulnerability check"
	echo "  --no-instruments     Skip Instruments tests"
	echo "  --no-cargo-shear    Skip cargo-shear unused dependency detection"
	echo "  --thread-sanitizer   Also run ThreadSanitizer tests (off by default;"
	echo "                      it instruments every build and is slow)"
	echo ""
	echo "Platform Support:"
	echo "  Linux:   Miri, Valgrind, cargo-deny, cargo-shear"
	echo "  macOS:   Miri, cargo-deny, cargo-shear, Instruments"
	echo ""
	echo "CI Environment:"
	echo "  The script automatically detects CI environments. Missing tools are"
	echo "  installed unless --no-auto-install is passed."
	echo ""
	echo "Tests performed:"
	echo "  - Miri: Detects undefined behavior in Rust code"
	echo "  - Valgrind: Memory error detection (Linux)"
	echo "  - ThreadSanitizer: Data race detection (opt-in)"
	echo "  - cargo-deny: Advisory, license, ban and source checks"
	echo "  - cargo-shear: Unused dependency detection (test targets included)"
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
		shift
		;;
	--no-auto-install)
		NO_AUTO_INSTALL=1
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
		RUN_CARGO_DENY=0
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
	--no-cargo-shear)
		NO_CARGO_SHEAR=1
		RUN_CARGO_SHEAR=0
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
	printf '%b\n' "${BLUE}${BOLD}$title${NC}" >&2
	printf '%b\n' "${BLUE}================================${NC}" >&2
	[[ -n $description ]] && echo "$description" >&2
	echo "" >&2
}

# OS detection with more robust platform detection
detect_platform() {
	if [[ $OSTYPE == "linux-gnu"* ]]; then
		OS_TYPE="Linux"
	elif [[ $OSTYPE == "darwin"* ]]; then
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
	if [[ -n ${CI:-} ]] || [[ -n ${GITHUB_ACTIONS:-} ]]; then
		IS_CI=1
		CI_PROVIDER="GitHub Actions"
		if [[ -z ${GITHUB_ACTIONS:-} ]]; then
			CI_PROVIDER="generic CI"
		fi

		# Deliberately does not re-enable installation: --no-auto-install must
		# survive CI detection, the only environment that flag exists for.
		if [[ $NO_AUTO_INSTALL -eq 1 ]]; then
			log_info "CI environment detected (${CI_PROVIDER}); auto-installation disabled by --no-auto-install"
		else
			log_info "CI environment detected (${CI_PROVIDER}); auto-installation enabled"
		fi
	else
		log_info "Interactive environment detected"
	fi
}

# Run with a time limit. `timeout` is GNU coreutils, which stock macOS lacks;
# Homebrew installs it as `gtimeout`. Without the fallback the Instruments
# probe fails on macOS and the trace security.yml uploads is never produced.
run_with_timeout() { # run_with_timeout <seconds> <command...>
	local seconds="$1"
	shift
	if have_cmd timeout; then
		timeout "${seconds}s" "$@"
	elif have_cmd gtimeout; then
		gtimeout "${seconds}s" "$@"
	else
		# No timeout available: run it anyway rather than skipping the check.
		"$@"
	fi
}

# True if xctrace is installed and actually responds. Probing it can be slow and
# may need a first-run permission prompt, hence the short limit.
instruments_available() {
	have_cmd xcrun || return 1
	run_with_timeout 10 xcrun xctrace version >/dev/null 2>&1
}

# Resolve the active toolchain once and remember whether it is nightly.
resolve_toolchain() {
	ACTIVE_TOOLCHAIN="$(rustup show active-toolchain 2>/dev/null | cut -d' ' -f1 || true)"
	[[ $ACTIVE_TOOLCHAIN == *"nightly"* ]] && HAS_NIGHTLY=1
	return 0
}

# Auto-configure tests based on platform capabilities
auto_configure_tests() {
	log_info "Auto-configuring tests for $OS_TYPE..."

	case "$OS_TYPE" in
	"Linux")
		log_info "  Linux detected: enabling Miri, Valgrind, cargo-deny and cargo-shear"
		# Instruments is a macOS-only tool.
		RUN_INSTRUMENTS=0
		RUN_CARGO_SHEAR=1
		;;
	"macOS")
		log_info "  macOS detected: enabling Miri, cargo-deny, cargo-shear and Instruments"
		log_info "  Disabling Valgrind (unreliable on macOS)"
		RUN_VALGRIND=0
		RUN_CARGO_SHEAR=1
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

# Detect available tools based on platform
detect_tools() {
	log_info "Detecting available security tools..."

	if have_cmd cargo-deny; then
		HAS_CARGO_DENY=1
		log_success "cargo-deny available"
	else
		log_warning "! cargo-deny not found"
	fi

	if have_cmd cargo-shear; then
		HAS_CARGO_SHEAR=1
		log_success "cargo-shear available"
	else
		log_warning "! cargo-shear not found"
	fi

	if [[ $OS_TYPE == "macOS" ]]; then
		# xctrace replaced the deprecated `instruments` CLI. Probing it is slow
		# and may need a first-run permission prompt, so keep the probe short.
		if instruments_available; then
			HAS_INSTRUMENTS=1
			log_success "Instruments (xctrace) available"
		else
			log_warning "! Instruments not found (will try to install Xcode in CI)"
		fi
	fi

	if [[ $OS_TYPE == "Linux" ]]; then
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
	[[ $OS_TYPE == "Linux" ]] &&
		printf '  %-30s %b\n' "Valgrind (memory errors)" "$(availability_badge "$HAS_VALGRIND")"
	printf '  %-30s %b\n' "cargo-deny (vulnerabilities)" "$(availability_badge "$HAS_CARGO_DENY")"
	[[ $OS_TYPE == "macOS" ]] &&
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
	# Report what will actually run. auto_configure_tests re-enables cargo-shear
	# on Linux and macOS after argument parsing, so RUN_CARGO_SHEAR alone would
	# claim "Enabled" for a suite that --no-cargo-shear switched off.
	local tsan_state shear_state
	tsan_state=$RUN_THREAD_SANITIZER
	shear_state=$RUN_CARGO_SHEAR
	[[ $NO_CARGO_SHEAR -eq 1 ]] && shear_state=0

	local flag
	for flag in "Miri:$RUN_MIRI" "Valgrind:$RUN_VALGRIND" "ThreadSanitizer:$tsan_state" \
		"cargo-deny:$RUN_CARGO_DENY" "Instruments:$RUN_INSTRUMENTS" "cargo-shear:$shear_state"; do
		if [[ ${flag#*:} -eq 1 ]]; then
			printf '  %-30s %b\n' "Run ${flag%%:*}" "${GREEN}Enabled${NC}"
		else
			printf '  %-30s %b\n' "Run ${flag%%:*}" "${YELLOW}Disabled${NC}"
		fi
	done

	echo ""
}

# Create security summary with tool outputs
declare -A TEST_RESULTS=()
SUITE_FAILURES=0

record_result() { # record_result <suite> <outcome> [detail]
	local detail="${3:-}"
	if [[ -n $detail ]]; then
		TEST_RESULTS["$1"]="$2 — $detail"
	else
		TEST_RESULTS["$1"]="$2"
	fi
}

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
		echo "| Tool | Status | Purpose |"
		echo "|------|--------|---------|"
		echo "| Miri | $(markdown_state "$HAS_MIRI" "Missing") | Undefined behaviour detection |"
		echo "| Valgrind | $(markdown_state "$HAS_VALGRIND" "Missing/N/A") | Memory error detection (Linux) |"
		echo "| ThreadSanitizer | $(markdown_state "$HAS_NIGHTLY" "Needs nightly") | Data race detection (built into rustc) |"
		echo "| cargo-deny | $(markdown_state "$HAS_CARGO_DENY" "Missing") | Advisory, licence, ban and source checks |"
		echo "| cargo-shear | $(markdown_state "$HAS_CARGO_SHEAR" "Missing") | Unused dependency detection |"
		echo "| Instruments | $(markdown_state "$HAS_INSTRUMENTS" "Missing/N/A") | Time Profiler trace (macOS) |"
		echo ""
		echo "## Test Results"
		echo ""
		echo "| Suite | Outcome |"
		echo "|-------|---------|"
		local suite outcome
		for suite in "${!TEST_RESULTS[@]}"; do
			outcome="${TEST_RESULTS[$suite]}"
			echo "| $suite | $outcome |"
		done | sort
		echo ""
		local failures="${SUITE_FAILURES:-0}"
		if [[ $failures -eq 0 ]]; then
			echo "All suites passed or were skipped."
		else
			echo "**$failures suite(s) failed.**"
		fi
		echo ""
		echo "## Logs"
		echo ""
		local log
		for log in "$LOG_DIR"/*.log; do
			[[ -f $log ]] || continue
			echo "- \`$(basename "$log")\`"
		done
		echo ""
	} >"$summary_file"
}

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

# Set up Xcode on macOS CI, then confirm xctrace really works: a present but
# unlicensed Xcode answers the probe slowly and uselessly.
install_xcode_ci() {
	if [[ $OS_TYPE != "macOS" ]] || [[ $IS_CI -ne 1 ]]; then
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

	if instruments_available; then
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

	if [[ $HAS_CARGO_DENY -eq 0 && $RUN_CARGO_DENY -eq 1 ]]; then
		log_info "Installing cargo-deny..."
		if cargo install --locked cargo-deny; then
			HAS_CARGO_DENY=1
			log_success "cargo-deny installed"
		else
			log_error "Failed to install cargo-deny"
		fi
	fi

	if [[ $NO_CARGO_SHEAR -eq 0 && $HAS_CARGO_SHEAR -eq 0 && $RUN_CARGO_SHEAR -eq 1 ]]; then
		log_info "Installing cargo-shear..."
		if cargo install --locked cargo-shear; then
			HAS_CARGO_SHEAR=1
			log_success "cargo-shear installed"
		else
			log_error "Failed to install cargo-shear"
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

	if [[ $OS_TYPE == "Linux" && $HAS_VALGRIND -eq 0 && $RUN_VALGRIND -eq 1 ]]; then
		if install_system_package valgrind; then
			HAS_VALGRIND=1
		fi
	fi

	if [[ $RUN_INSTRUMENTS -eq 1 && $HAS_INSTRUMENTS -eq 0 ]]; then
		install_xcode_ci || true
	fi

	echo ""
}

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
	if [[ -d $TEST_TARGET_PATH ]]; then
		echo "check $TEST_TARGET_PATH"
	else
		echo "--help"
	fi
}

log_path() { echo "$LOG_DIR/$1_${TIMESTAMP}.log"; }

# Build rustowlc, the RUSTC wrapper rustowl drives, and print its path.
#
# Without it rustowl cannot resolve an analysis target and logs "Invalid
# analysis target" then "Analyze failed" -- so the memory suites would appear to
# pass while analysing nothing. rustowl picks it up from the RUSTOWLC env var.
#
# Built without the sanitizer on purpose: rustowlc only spawns rustc, so
# instrumenting it adds nothing and risks TSAN noise across its IPC.
# ensure_rustowlc [toolchain-channel]
#
# Must go through scripts/toolchain, the same wrapper that builds the rustowl
# binary driving it. rustowlc links librustc_driver-*.so out of that toolchain's
# sysroot, and rustowl hands rustowlc a --sysroot for the toolchain baked into
# *itself*. Build the two with different toolchains and rustowlc dies at load
# time with "cannot open shared object file", which surfaces as a bare
# "cargo metadata exited with 101".
#
# Pass a channel when the suite needs a toolchain other than the default one --
# ThreadSanitizer needs a nightly for -Zsanitizer.
# The toolchain channel rust-toolchain.toml pins, which is what -Zsanitizer has
# to build with. Read from the file rather than hardcoded so bumping the pin
# cannot leave this suite building with a stale nightly.
pinned_toolchain_channel() {
	local channel
	channel="$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ { print $2; exit }' \
		"$REPO_ROOT/rust-toolchain.toml" 2>/dev/null || true)"
	if [[ -z $channel ]]; then
		log_error "Could not read the channel from rust-toolchain.toml"
		return 1
	fi
	printf '%s\n' "$channel"
}

ensure_rustowlc() {
	local channel="${1:-}"
	local out path
	if [[ -n $channel ]]; then
		out="$(TOOLCHAIN_CHANNEL="$channel" RUST_COMPONENTS=rust-src \
			./scripts/toolchain cargo build --bin rustowlc --message-format=json 2>&1)" || {
			log_error "Failed to build rustowlc with $channel"
			printf '%s\n' "$out" | tail -5 >&2
			return 1
		}
	else
		out="$(./scripts/toolchain cargo build --bin rustowlc --message-format=json 2>&1)" || {
			log_error "Failed to build rustowlc"
			printf '%s\n' "$out" | tail -5 >&2
			return 1
		}
	fi

	path="$(printf '%s\n' "$out" |
		grep -o '"executable":"[^"]*"' | head -1 | cut -d'"' -f4)"

	if [[ -z $path || ! -f $path ]]; then
		log_error "cargo reported no usable executable for rustowlc"
		return 1
	fi

	echo "$path"
}

# Locate the release rustowl binary, building it if necessary.
rustowl_release_binary() {
	local binary="./target/release/rustowl"

	if [[ ! -f $binary ]]; then
		log_info "Building RustOwl for this test..."
		if ! ./scripts/toolchain cargo build --release >/dev/null 2>&1; then
			log_error "Failed to build RustOwl"
			return 1
		fi
	fi

	if [[ ! -f $binary ]]; then
		log_error "RustOwl binary not found at $binary"
		return 1
	fi

	echo "$binary"
}

run_miri_tests() {
	[[ $RUN_MIRI -eq 1 ]] || {
		record_result Miri "Skipped (--no-miri)"
		return 0
	}

	if [[ $HAS_MIRI -eq 0 ]]; then
		log_warning "Skipping Miri tests (component not installed)"
		record_result Miri "Skipped (Miri not installed)"
		return 0
	fi

	print_section_header "Running Miri Tests" "Miri detects undefined behavior in Rust code"

	# These flags are what let RustOwl spawn the cargo/rustc processes it needs.
	local miri_env='MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-permissive-provenance" RUSTFLAGS="--cfg miri"'
	local args analysis_log analysis_desc
	args="$(analysis_args)"

	log_info "Running RustOwl unit tests with Miri..."
	if run_logged miri_unit_tests "$miri_env cargo miri test --lib"; then
		record_result Miri "Passed"
		log_success "RustOwl unit tests passed with Miri"
	else
		record_result Miri "FAILED" "unit tests failed under Miri"
		log_error "RustOwl unit tests failed with Miri"
		log_info "  Full output captured in: $(log_path miri_unit_tests)"
		return 1
	fi

	# Prefer the real analysis run; fall back to --help when the test package is
	# absent, so there is always something for Miri to execute.
	if [[ -d $TEST_TARGET_PATH ]]; then
		analysis_log="miri_rustowl_analysis"
		analysis_desc="RustOwl analysis"
	else
		# analysis_args already resolved to --help; only the naming differs.
		analysis_log="miri_basic_execution"
		analysis_desc="basic RustOwl execution"
		log_warning "No test target found at $TEST_TARGET_PATH; falling back to --help"
	fi

	log_info "Testing ${analysis_desc} with Miri..."
	if run_logged "$analysis_log" "$miri_env cargo miri run --bin rustowl -- $args"; then
		record_result Miri "Passed" "unit tests and $analysis_desc"
		log_success "${analysis_desc} completed with Miri"
	else
		# A real Miri limitation, not a regression: RustOwl spawns cargo, which
		# Miri cannot do. So the unit tests still stand and the build must not
		# fail, but the outcome has to say the analysis never ran rather than
		# leaving the earlier bare "Passed" standing.
		record_result Miri "Passed (unit tests only)" \
			"$analysis_desc did not run: RustOwl spawns cargo, which Miri cannot do"
		log_warning "Miri could not complete ${analysis_desc}: RustOwl spawns cargo processes"
		log_warning "  Only the unit tests were validated; the analysis step is unsupported under Miri"
		log_info "  Full output captured in: $(log_path "$analysis_log")"
	fi

	echo ""
}

run_thread_sanitizer_tests() {
	[[ $RUN_THREAD_SANITIZER -eq 1 ]] || {
		record_result ThreadSanitizer "Skipped (not requested)"
		return 0
	}

	print_section_header "Running ThreadSanitizer Tests" \
		"ThreadSanitizer detects data races and threading issues"

	if [[ $HAS_NIGHTLY -eq 0 ]]; then
		# No log file is written on this path, so do not name one.
		log_warning "ThreadSanitizer needs a nightly toolchain (active: ${ACTIVE_TOOLCHAIN:-unknown}); skipping"
		record_result ThreadSanitizer "Skipped (stable toolchain)"
		return 0
	fi

	local args
	args="$(analysis_args)"

	# All three flags are load-bearing. -Zbuild-std because -Zsanitizer changes the
	# crate ABI, so core must be rebuilt with the same flag; --target because
	# without it build scripts link an uninstrumented std and fail one level up.
	# No +nightly: rust-toolchain.toml already pins a dated nightly.
	local target
	target="$(rustc -vV | awk '/^host:/ { print $2 }')"

	# report_thread_leaks (not detect_thread_leaks — TSAN has no such flag and
	# ignores it silently) is off because tokio's blocking-pool workers can still
	# be parked when a short-lived CLI exits. That is not a RustOwl defect, and
	# it says nothing about races.
	local tsan_options="suppressions=$REPO_ROOT/.tsan-suppressions:report_thread_leaks=0"
	if [[ ! -f "$REPO_ROOT/.tsan-suppressions" ]]; then
		tsan_options="report_thread_leaks=0"
	fi

	# Build through scripts/toolchain, like every other rustowl build here, and
	# hand ensure_rustowlc the same channel. Building rustowl with bare cargo
	# baked RUSTOWL_TOOLCHAIN from the rustup nightly, so at run time rustowl
	# found no ~/.rustowl/sysroot for it and downloaded one -- and the rustowlc
	# built for the default channel could not find its librustc_driver in it.
	# -Zsanitizer needs a nightly, so the channel comes from rust-toolchain.toml.
	local tsan_channel
	tsan_channel="$(pinned_toolchain_channel)" || return 1

	log_info "Using RUSTFLAGS: -Zsanitizer=thread, target: $target"
	log_info "Building with scripts/toolchain channel: $tsan_channel"

	# Two steps because an aborted run emits zero TSAN warnings, so a verdict
	# from the warnings alone would call a broken build "no races detected".
	local build_output binary rustowlc
	if ! build_output="$(TOOLCHAIN_CHANNEL="$tsan_channel" RUST_COMPONENTS=rust-src \
		RUSTFLAGS="-Zsanitizer=thread" ./scripts/toolchain cargo build -Zbuild-std \
		--target "$target" --bin rustowl --message-format=json 2>&1)"; then
		mkdir -p "$LOG_DIR"
		printf '%s\n' "$build_output" >"$(log_path tsan_build)"
		log_error "Failed to build RustOwl with ThreadSanitizer; no race check was performed"
		log_info "  Full output captured in: $(log_path tsan_build)"
		return 1
	fi

	# Ask cargo where the artifact went: a hardcoded path is wrong whenever
	# CARGO_TARGET_DIR is set, which the CI job does.
	binary="$(printf '%s\n' "$build_output" |
		grep -o '"executable":"[^"]*"' | head -1 | cut -d'"' -f4)"

	if [[ -z $binary || ! -f $binary ]]; then
		log_error "cargo reported no usable executable for the instrumented build"
		return 1
	fi

	if ! rustowlc="$(ensure_rustowlc "$tsan_channel")"; then
		record_result ThreadSanitizer "FAILED" "could not build rustowlc"
		return 1
	fi

	# Bounded because TSan runs do not always converge: tokio's `test_tuning`
	# needs killing after ~27 minutes under the sanitizer.
	local tsan_log status=0
	run_logged tsan_rustowl_analysis \
		"TSAN_OPTIONS='$tsan_options' RUSTOWLC='$rustowlc' run_with_timeout 300 '$binary' $args" ||
		status=$?
	tsan_log="$(log_path tsan_rustowl_analysis)"

	# With the build known good, the sanitizer output is trustworthy.
	# `rustowl check` exits non-zero when it reports findings, so its exit
	# code says nothing about races.
	if [[ -f $tsan_log ]] && grep -q "WARNING: ThreadSanitizer" "$tsan_log"; then
		local races
		races=$(grep -c "WARNING: ThreadSanitizer" "$tsan_log" || echo 0)
		record_result ThreadSanitizer "FAILED" "$races finding(s)"
		log_error "ThreadSanitizer reported $races finding(s)"
		log_info "  Full output captured in: $tsan_log"
		return 1
	fi

	if [[ $status -eq 124 ]]; then
		record_result ThreadSanitizer "FAILED" "did not converge within 300s"
		log_error "The instrumented run did not finish within 300s and was killed"
		log_info "  Partial output captured in: $tsan_log"
		return 1
	fi

	# Any other non-zero status with no race report is the analysis failing,
	# not a clean run: a missing rustowlc exits 127, an aborted launch exits
	# something else. Recording a pass there reports a check that never ran.
	if [[ $status -ne 0 ]]; then
		record_result ThreadSanitizer "FAILED" "analysis exited $status"
		log_error "The instrumented analysis exited $status without reporting a race"
		log_warning "  This is an analysis failure, not a data race -- check the log for the cause"
		log_info "  Partial output captured in: $tsan_log"
		return 1
	fi

	record_result ThreadSanitizer "Passed" "no races detected"
	log_success "RustOwl analysis completed under ThreadSanitizer (no races detected)"

	echo ""
}

run_valgrind_tests() {
	[[ $RUN_VALGRIND -eq 1 ]] || {
		record_result Valgrind "Skipped (--no-valgrind)"
		return 0
	}

	if [[ $HAS_VALGRIND -eq 0 ]]; then
		log_warning "Skipping Valgrind tests (not available on this platform)"
		record_result Valgrind "Skipped (not available)"
		return 0
	fi

	print_section_header "Running Valgrind Tests" \
		"Valgrind detects memory errors, leaks, and memory corruption"

	local binary rustowlc
	if ! binary="$(rustowl_release_binary)"; then
		record_result Valgrind "FAILED" "rustowl binary unavailable"
		return 1
	fi
	if ! rustowlc="$(ensure_rustowlc)"; then
		record_result Valgrind "FAILED" "could not build rustowlc"
		return 1
	fi

	local suppressions=""
	if [[ -f ".valgrind-suppressions" ]]; then
		suppressions="--suppressions=$REPO_ROOT/.valgrind-suppressions"
		log_info "Using suppressions file: $REPO_ROOT/.valgrind-suppressions"
	fi

	# --error-exitcode only fires when valgrind itself counts an error, so this
	# sentinel separates "valgrind found memory errors" from "the program under
	# test exited non-zero". Without it valgrind exits 0 regardless of what it
	# finds, and the suite's verdict is just RustOwl's exit code. Only definite
	# and indirect leaks count; reachable and possible are live statics and
	# thread-local storage, which .valgrind-suppressions already targets.
	local flags="valgrind --tool=memcheck --leak-check=full --show-leak-kinds=all --track-origins=yes"
	flags+=" --errors-for-leak-kinds=definite,indirect --error-exitcode=$VALGRIND_ERROR_EXIT"
	flags+=" $suppressions"

	local args
	args="$(analysis_args)"

	log_info "Running RustOwl with Valgrind..."
	log_info "Using Valgrind flags: --tool=memcheck --leak-check=full --show-leak-kinds=all"
	log_info "Leak kinds counted as errors: definite, indirect"

	local status=0
	run_logged valgrind_analysis "RUSTOWLC='$rustowlc' $flags $binary $args" || status=$?

	if [[ $status -eq 0 ]]; then
		record_result Valgrind "Passed" "no memory errors detected"
		log_success "RustOwl analysis completed with Valgrind (no memory errors detected)"
	elif [[ $status -eq $VALGRIND_ERROR_EXIT ]]; then
		record_result Valgrind "FAILED" "valgrind reported memory errors"
		log_error "Valgrind detected memory errors in RustOwl analysis"
		log_info "  Full output captured in: $(log_path valgrind_analysis)"
		return 1
	else
		# RustOwl exited non-zero but valgrind counted nothing: the analysis
		# itself failed, which is a different problem from a memory error.
		record_result Valgrind "FAILED" "rustowl exited $status under valgrind, no memory errors"
		log_error "RustOwl exited $status under Valgrind (valgrind reported no memory errors)"
		log_warning "This is an analysis failure, not a memory error -- check the log for the cause"
		log_info "  Full output captured in: $(log_path valgrind_analysis)"
		return 1
	fi

	echo ""
}

run_instruments_tests() {
	[[ $RUN_INSTRUMENTS -eq 1 ]] || {
		record_result Instruments "Skipped (--no-instruments or unsupported platform)"
		return 0
	}

	if [[ $HAS_INSTRUMENTS -eq 0 ]]; then
		log_warning "Skipping Instruments tests (not available on this platform)"
		record_result Instruments "Skipped (xctrace unavailable)"
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
		if [[ -d $trace || -f $trace ]]; then
			record_result Instruments "Passed" "trace captured"
			log_success "Instruments trace captured: $trace"
		else
			record_result Instruments "FAILED" "no trace produced"
			log_error "xctrace reported success but produced no trace at $trace"
			return 1
		fi
	else
		record_result Instruments "FAILED" "trace recording failed"
		log_error "Instruments failed to record a trace"
		log_info "  Full output captured in: $(log_path instruments_analysis)"
		return 1
	fi

	echo ""
}

run_audit_check() {
	if [[ $RUN_CARGO_DENY -eq 0 ]]; then
		record_result cargo-deny "Skipped (--no-audit)"
		return 0
	fi

	if [[ $HAS_CARGO_DENY -eq 0 ]]; then
		log_warning "Skipping cargo-deny (not installed)"
		record_result cargo-deny "Skipped (not installed)"
		return 0
	fi

	log_info "Scanning dependencies for vulnerabilities..."
	if cargo deny check advisories; then
		record_result cargo-deny "Passed" "no advisories found"
		log_success "No known vulnerabilities found"
	else
		record_result cargo-deny "FAILED" "advisories reported"
		log_error "Security vulnerabilities detected"
		return 1
	fi

	echo ""
}

run_cargo_machete_tests() {
	[[ $NO_CARGO_SHEAR -eq 0 && $RUN_CARGO_SHEAR -eq 1 ]] || {
		record_result cargo-shear "Skipped (--no-cargo-shear)"
		return 0
	}

	if [[ $HAS_CARGO_SHEAR -eq 0 ]]; then
		log_warning "Skipping cargo-shear tests (not installed)"
		record_result cargo-shear "Skipped (not installed)"
		return 0
	fi

	print_section_header "Running cargo-shear Tests" \
		"cargo-shear detects unused dependencies in Cargo.toml"

	log_info "Scanning for unused dependencies..."

	# cargo-shear exits non-zero when it finds unused dependencies, which is a
	# warning for us rather than a suite failure, so never let it fail the run.
	if run_logged cargo_shear_analysis "cargo shear --check-test-targets"; then
		log_success "cargo-shear analysis completed"
	else
		log_warning "cargo-shear found potential issues"
		log_warning "  Note: cargo-shear may report false positives for conditionally used deps"
	fi

	local log_file
	log_file="$(log_path cargo_shear_analysis)"
	if [[ -f $log_file ]] && grep -q "unused dependencies" "$log_file" 2>/dev/null; then
		log_warning "Found potential unused dependencies - check $log_file for details"
	else
		log_success "No unused dependencies detected"
	fi

	echo ""
}

main() {
	print_section_header "RustOwl Security & Memory Safety Testing" ""

	detect_platform
	detect_ci_environment

	if [[ $MODE == "check" ]]; then
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

	# Same order as the --check path: apply platform defaults, then re-detect
	# against them, so what is reported is what will actually run.
	auto_configure_tests
	detect_tools

	# --install is an explicit request and always wins. Otherwise only a
	# detected CI run installs, and --no-auto-install vetoes even that.
	if [[ $MODE == "install" ]] ||
		{ [[ $IS_CI -eq 1 ]] && [[ $NO_AUTO_INSTALL -eq 0 ]]; }; then
		install_required_tools
		# Re-detect after installing.
		detect_tools
	fi

	check_rust_version "$MIN_RUST_VERSION"

	# Report the final, post-install configuration before running anything.
	show_tool_status

	echo ""
	log_info "Running security tests..."
	echo ""

	# Written up front so the file exists even if a suite aborts the run; the
	# real one with per-suite outcomes is written after the suites finish.
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

	# Written after the suites so it can report what each one actually did, not
	# just which tools were present beforehand.
	SUITE_FAILURES="$test_failures"
	create_security_summary

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
