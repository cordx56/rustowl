#!/usr/bin/env bash
# Local performance benchmarking script for RustOwl
# Runs Criterion benchmarks locally, with comparison and regression detection.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=scripts/lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

cd "$REPO_ROOT"

setup_nix_ld_paths

# Configuration
BENCHMARK_NAME="rustowl_bench_simple"

# Look for existing test packages in the repo
TEST_PACKAGES=(
	"./tests/fixtures"
	"./crates/rustowl/benches/fixtures"
	"./test-data"
	"./examples"
	"./crates/rustowl/perf-tests"
)

# Options
OPEN_REPORT=false
SAVE_BASELINE=""
LOAD_BASELINE=""
COMPARE_MODE=false
CLEAN_BUILD=false
SHOW_OUTPUT=true
REGRESSION_THRESHOLD="5%"
# Parsed once from REGRESSION_THRESHOLD and validated, so the comparison never
# re-parses user input into an arithmetic expression. Units are tenths of a
# percent, matching pct_change_tenths.
REGRESSION_THRESHOLD_TENTHS=50
TEST_PACKAGE_PATH=""

info() {
	[[ "$SHOW_OUTPUT" == "true" ]] && printf '%b\n' "${YELLOW}$1${NC}"
	return 0
}
note() {
	[[ "$SHOW_OUTPUT" == "true" ]] && printf '%b\n' "${GREEN}$1${NC}"
	return 0
}
warn() {
	[[ "$SHOW_OUTPUT" == "true" ]] && printf '%b\n' "${RED}$1${NC}"
	return 0
}
detail() {
	[[ "$SHOW_OUTPUT" == "true" ]] && printf '%b\n' "$1"
	return 0
}

usage() {
	echo "Usage: $0 [OPTIONS]"
	echo ""
	echo "Performance Benchmarking Script for RustOwl"
	echo "Runs Criterion benchmarks with comparison and regression detection capabilities"
	echo ""
	echo "Options:"
	echo "  -h, --help           Show this help message"
	echo "  --save <name>        Save benchmark results as baseline with given name"
	echo "  --load <name>        Load baseline and compare current results against it"
	echo "  --threshold <percent> Set regression threshold (default: 5%)"
	echo "  --test-package <path> Use specific test package (auto-detected if not specified)"
	echo "  --open               Open HTML report in browser after benchmarking"
	echo "  --clean              Clean build artifacts before benchmarking"
	echo "  --quiet              Minimal output (for CI/automated use)"
	echo ""
	echo "Examples:"
	echo "  $0                           # Run benchmarks with default settings"
	echo "  $0 --save main               # Save results as 'main' baseline"
	echo "  $0 --load main --threshold 3% # Compare against 'main' with 3% threshold"
	echo "  $0 --clean --open            # Clean build, run benchmarks, open report"
	echo "  $0 --save current --quiet    # Save baseline quietly (for CI)"
	echo ""
	echo "Baseline Management:"
	echo "  Baselines are stored in: baselines/performance/<name>/"
	echo "  HTML reports are in: target/criterion/report/"
	echo ""
}

require_value() { # require_value <flag> <value> <example>
	if [[ -z "${2:-}" ]]; then
		warn "Error: $1 requires a value"
		detail "Example: $0 $3"
		exit 1
	fi
}

# Accept a whole number of percent, with an optional trailing %.
validate_threshold() { # validate_threshold <value>
	local value="${1//%/}"
	case "$value" in
	'' | *[!0-9]*)
		warn "Error: --threshold must be a whole number of percent, got: $1"
		detail "Example: $0 --threshold 3%"
		exit 1
		;;
	esac
	# Strip leading zeros so the value is never read as octal ("08" would be
	# rejected as an invalid octal literal inside $(( ))).
	REGRESSION_THRESHOLD_TENTHS=$((10#${value#0} * 10))
	# Normalise the display form so "--threshold 7" and "--threshold 7%" report
	# identically in the header and in benchmark-summary.txt.
	REGRESSION_THRESHOLD="${value#0}%"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
	case $1 in
	-h | --help)
		usage
		exit 0
		;;
	--save)
		require_value "$1" "${2:-}" "--save main"
		SAVE_BASELINE="$2"
		shift 2
		;;
	--load)
		require_value "$1" "${2:-}" "--load main"
		LOAD_BASELINE="$2"
		COMPARE_MODE=true
		shift 2
		;;
	--threshold)
		require_value "$1" "${2:-}" "--threshold 3%"
		REGRESSION_THRESHOLD="$2"
		validate_threshold "$2"
		shift 2
		;;
	--test-package)
		require_value "$1" "${2:-}" "--test-package ./examples/sample"
		TEST_PACKAGE_PATH="$2"
		shift 2
		;;
	--open)
		OPEN_REPORT=true
		shift
		;;
	--clean)
		CLEAN_BUILD=true
		shift
		;;
	--quiet)
		SHOW_OUTPUT=false
		shift
		;;
	baseline)
		# Legacy support for CI workflow
		SAVE_BASELINE="main"
		SHOW_OUTPUT=false
		shift
		;;
	compare)
		# Legacy support for CI workflow
		COMPARE_MODE=true
		LOAD_BASELINE="main"
		shift
		;;
	*)
		warn "Unknown option: $1"
		detail "Use --help for usage information"
		exit 1
		;;
	esac
done

# Milliseconds since the epoch, or nothing where date(1) has no %N (BSD).
now_ms() {
	local ns
	ns="$(date +%s%N 2>/dev/null)" || return 0
	case "$ns" in
	*N* | '') return 0 ;; # %N unsupported: BSD date echoes the format literally
	esac
	echo $((ns / 1000000))
}

# Render milliseconds as seconds: 1234 -> 1.234
format_ms() {
	printf '%d.%03d' "$(($1 / 1000))" "$(($1 % 1000))"
}

# Read a stored duration ("1.234", "1.2" or "2") back as milliseconds, so
# baselines written by older versions still compare.
seconds_to_ms() {
	local value="$1" whole frac
	whole="${value%%.*}"
	frac="${value#*.}"
	[ "$frac" = "$value" ] && frac=""
	while [ "${#frac}" -lt 3 ]; do frac="${frac}0"; done
	printf '%d' "$((whole * 1000 + 10#${frac:0:3}))"
}

current_mode_description() {
	if [[ -n "$SAVE_BASELINE" ]]; then
		echo "Save baseline ($SAVE_BASELINE)"
	elif [[ "$COMPARE_MODE" == "true" ]]; then
		echo "Compare against $LOAD_BASELINE"
	else
		echo "Standard run"
	fi
}

# The header both writers of benchmark-summary.txt need. They used to inline
# these same four lines, which is exactly what jscpd flagged as a clone.
write_summary_header() {
	echo "# RustOwl Benchmark Summary"
	echo "Generated: $(date)"
	echo "Test Package: $TEST_PACKAGE_PATH"
	echo "Mode: $(current_mode_description)"
}

print_header() {
	if [[ "$SHOW_OUTPUT" == "true" ]]; then
		printf '%b\n' "${BLUE}${BOLD}=====================================${NC}"
		printf '%b\n' "${BLUE}${BOLD}  RustOwl Performance Benchmarks${NC}"
		printf '%b\n' "${BLUE}${BOLD}=====================================${NC}"
		echo ""

		if [[ -n "$SAVE_BASELINE" ]]; then
			note "Mode: Save baseline as '$SAVE_BASELINE'"
		elif [[ "$COMPARE_MODE" == "true" ]]; then
			note "Mode: Compare against '$LOAD_BASELINE' baseline"
			note "Regression threshold: $REGRESSION_THRESHOLD"
		else
			note "Mode: Standard benchmark run"
		fi
		echo ""
	fi
}

find_test_package() {
	if [[ -n "$TEST_PACKAGE_PATH" ]]; then
		if [[ -d "$TEST_PACKAGE_PATH" ]]; then
			note "✓ Using specified test package: $TEST_PACKAGE_PATH"
			return 0
		fi
		warn "Error: Specified test package not found: $TEST_PACKAGE_PATH"
		exit 1
	fi

	# Auto-detect existing test packages
	local test_dir
	for test_dir in "${TEST_PACKAGES[@]}"; do
		[[ -d "$test_dir" ]] || continue

		# A directory of Rust sources is a package in its own right...
		if find "$test_dir" -name "*.rs" -print -quit | grep -q .; then
			TEST_PACKAGE_PATH="$test_dir"
			note "✓ Found test package: $TEST_PACKAGE_PATH"
			return 0
		fi

		# ...otherwise fall back to the first nested package we can find.
		local nested
		nested="$(find "$test_dir" -name "Cargo.toml" -print -quit)"
		if [[ -n "$nested" ]]; then
			TEST_PACKAGE_PATH="$(dirname "$nested")"
			note "✓ Found test package: $TEST_PACKAGE_PATH"
			return 0
		fi
	done

	# Look for existing benchmark files
	if [[ -d "./crates/rustowl/benches" ]]; then
		TEST_PACKAGE_PATH="./crates/rustowl/benches"
		note "✓ Using benchmark directory: $TEST_PACKAGE_PATH"
		return 0
	fi

	# Use the current project as test package
	if [[ -f "./Cargo.toml" ]]; then
		TEST_PACKAGE_PATH="."
		note "✓ Using current project as test package"
		return 0
	fi

	warn "Error: No suitable test package found in the repository"
	detail "Searched in: ${TEST_PACKAGES[*]}"
	detail "Use --test-package <path> to specify a custom location"
	exit 1
}

check_prerequisites() {
	info "Checking prerequisites..."

	# Check Rust installation (any version is fine - we trust rust-toolchain.toml)
	if ! have_cmd rustc; then
		warn "Error: Rust is not installed"
		detail "Please install Rust: https://rustup.rs/"
		exit 1
	fi

	note "✓ Rust: $(rustc --version)"
	note "✓ Cargo: $(cargo --version)"
	note "✓ Host: $(rustc -vV | awk '/^host:/ { print $2 }')"

	if have_cmd cargo-criterion; then
		note "✓ cargo-criterion is available"
	else
		info "! cargo-criterion not found, using cargo bench"
	fi

	find_test_package

	detail ""
}

clean_build() {
	if [[ "$CLEAN_BUILD" == "true" ]]; then
		info "Cleaning build artifacts..."
		./scripts/toolchain cargo clean
		note "✓ Build artifacts cleaned"
		detail ""
	fi
}

build_rustowl() {
	info "Building RustOwl in release mode..."

	local -a args=(cargo build --release)
	[[ "$SHOW_OUTPUT" == "true" ]] || args+=(--quiet)
	./scripts/toolchain "${args[@]}"

	note "✓ Build completed"
	detail ""
}

run_criterion_benchmarks() {
	[[ -d "./crates/rustowl/benches" ]] || return 0
	find "./crates/rustowl/benches" -name "*.rs" -print -quit | grep -q . || {
		info "! No benchmark files found in ./crates/rustowl/benches, skipping Criterion benchmarks"
		return 0
	}

	local -a bench_cmd=(./scripts/toolchain cargo bench)
	local -a bench_args=()

	# cargo-criterion only makes sense for a plain run, not for baseline I/O.
	if have_cmd cargo-criterion && [[ -z "$SAVE_BASELINE" && "$COMPARE_MODE" != "true" ]]; then
		bench_cmd=(./scripts/toolchain cargo criterion)
	fi

	if [[ -n "$SAVE_BASELINE" ]]; then
		bench_args+=(--bench "$BENCHMARK_NAME" -- --save-baseline "$SAVE_BASELINE")
	elif [[ "$COMPARE_MODE" == "true" && -n "$LOAD_BASELINE" ]]; then
		bench_args+=(--bench "$BENCHMARK_NAME" -- --baseline "$LOAD_BASELINE")
	else
		bench_args+=(--bench "$BENCHMARK_NAME")
	fi

	info "Running performance benchmarks..."

	local bench_status=0
	if [[ "$SHOW_OUTPUT" == "true" ]]; then
		detail "${bench_cmd[*]} ${bench_args[*]}"
		"${bench_cmd[@]}" "${bench_args[@]}" || bench_status=$?
	else
		"${bench_cmd[@]}" "${bench_args[@]}" >/dev/null 2>&1 || bench_status=$?
	fi

	# Criterion exiting non-zero is how it reports a regression; our own
	# comparison below decides the verdict, so don't abort the whole run.
	if [[ "$bench_status" -ne 0 ]]; then
		info "! Criterion exited with status $bench_status"
	fi
}

run_analysis_benchmark() {
	if [[ ! -f "./target/release/rustowl" && ! -f "./target/release/rustowl.exe" ]]; then
		info "! RustOwl binary not found, skipping analysis benchmark"
		return 0
	fi

	local rustowl_binary="./target/release/rustowl"
	if [[ -f "./target/release/rustowl.exe" ]]; then
		rustowl_binary="./target/release/rustowl.exe"
	fi

	info "Running RustOwl analysis benchmark on: $TEST_PACKAGE_PATH"

	local start_ms end_ms duration
	start_ms="$(now_ms)"
	timeout 120 "$rustowl_binary" check "$TEST_PACKAGE_PATH" >/dev/null 2>&1 || true
	end_ms="$(now_ms)"

	if [[ -n "$start_ms" && -n "$end_ms" ]]; then
		duration="$(format_ms "$((end_ms - start_ms))")"
	else
		duration="N/A"
	fi

	note "✓ Analysis completed in ${duration}s"

	if [[ -n "$SAVE_BASELINE" ]]; then
		local dir="baselines/performance/$SAVE_BASELINE"
		mkdir -p "$dir"
		{
			echo "$duration"
		} >"$dir/analysis_time.txt"
		echo "$TEST_PACKAGE_PATH" >"$dir/test_package.txt"
		# Copy Criterion benchmark results for local development
		if [[ -d "target/criterion" ]]; then
			cp -r "target/criterion" "$dir/criterion"
		fi
	fi

	if [[ "$COMPARE_MODE" == "true" && -f "baselines/performance/$LOAD_BASELINE/analysis_time.txt" ]]; then
		local baseline_time
		baseline_time="$(cat "baselines/performance/$LOAD_BASELINE/analysis_time.txt")"
		compare_analysis_times "$baseline_time" "$duration"
	fi
}

run_benchmarks() {
	run_criterion_benchmarks
	run_analysis_benchmark
	note "✓ Benchmarks completed"
	detail ""
}

compare_analysis_times() {
	local baseline_time="$1"
	local current_time="$2"

	if [[ "$baseline_time" == "N/A" || "$current_time" == "N/A" ]]; then
		info "! Could not compare analysis times (timing unavailable)"
		return 0
	fi

	# Signed tenths of a percent, so the threshold comparison is integer-only.
	# Sign and magnitude are kept apart: the comparison must be signed (an
	# improvement has a negative change), while the prose wants a magnitude.
	local change magnitude sign threshold_tenths
	change="$(pct_change_tenths "$(seconds_to_ms "$baseline_time")" "$(seconds_to_ms "$current_time")")"
	if [[ "${change#-}" != "$change" ]]; then
		sign="-"
	else
		# No leading '+', matching what the old bc-formatted output printed.
		sign=""
	fi
	magnitude="${change#-}"
	threshold_tenths=$((REGRESSION_THRESHOLD_TENTHS))

	detail "Analysis Time Comparison:"
	detail "  Baseline: ${baseline_time}s"
	detail "  Current:  ${current_time}s"
	detail "  Change:   $(format_tenths "$magnitude" "$sign")%"
	detail ""

	# A slowdown is a regression. Compare the SIGNED value, otherwise a large
	# improvement matches "magnitude > threshold" and is reported as a
	# regression — which also aborted the run under `set -e`, so no summary was
	# ever written.
	if ((change > threshold_tenths)); then
		warn "⚠ Performance regression detected! (+$(format_tenths "$magnitude" "")% > ${REGRESSION_THRESHOLD})"
		return 1
	fi
	if ((change < -threshold_tenths)); then
		note "✓ Performance improvement detected! ($(format_tenths "$magnitude" "")%)"
	else
		note "✓ Performance within acceptable range (±$(format_tenths "$threshold_tenths" "")%)"
	fi
}

# Extract criterion timings into the summary file.
write_criterion_details() {
	local criterion_dir="$1"

	if have_cmd jq; then
		{
			echo "### Detailed Timings (JSON extracted)"
			find "$criterion_dir" -name "estimates.json" -exec bash -c '
                dir=$(dirname "$1" | sed "s|target/criterion/||")
                val=$(jq -r ".mean.point_estimate" "$1" 2>/dev/null || echo "N/A")
                if [ "$val" != "N/A" ] && [ "$val" != "null" ]; then
                    # Convert nanoseconds to seconds with 3 decimal places
                    sec=$(printf "%d.%03d" $((val / 1000000000)) $((val % 1000000000 / 1000000)))
                    echo "$dir: ${sec}s"
                else
                    echo "$dir: N/A"
                fi' bash {} \; | sort
		} >>benchmark-summary.txt 2>/dev/null || true

		local measurement_time
		measurement_time="$(find "$criterion_dir" -name "estimates.json" -exec jq -r '.measurement_time' {} + 2>/dev/null | head -1)"
		[[ -n "$measurement_time" && "$measurement_time" != "null" ]] || measurement_time=300

		{
			echo ""
			echo "### Summary Statistics"
			echo "Sample Size: $(find "$criterion_dir" -name "sample.json" | head -1 | xargs jq -r 'length' 2>/dev/null || echo 'N/A') measurements per benchmark"
			echo "Measurement Time: ${measurement_time}s per benchmark"
			echo "Warm-up Time: 5s per benchmark"
		} >>benchmark-summary.txt
	else
		{
			echo "### Quick Summary (grep extracted)"
			find "$criterion_dir" -name "*.json" -exec grep -h "\"mean\"" {} \; 2>/dev/null | head -10
		} >>benchmark-summary.txt 2>/dev/null || true
	fi
}

analyze_regressions() {
	if [[ "$COMPARE_MODE" != "true" ]]; then
		return 0
	fi

	info "Analyzing benchmark results for regressions..."

	local criterion_dir="target/criterion"
	local regression_found=false

	[[ -d "$criterion_dir" ]] || return 0

	# Only do the detailed HTML scan in non-verbose (CI) mode
	if [[ "$SHOW_OUTPUT" == "false" ]]; then
		if find "$criterion_dir" -name "*.html" -print0 2>/dev/null |
			xargs -0 grep -l "regressed\|slower" 2>/dev/null |
			head -1 | grep -q .; then
			regression_found=true
		fi
	fi

	if [[ -f "$criterion_dir/report/index.html" ]]; then
		{
			write_summary_header
			echo ""
			echo "## Reports Available"
			echo "- HTML Report: target/criterion/report/index.html"
			find "$criterion_dir" -name "index.html" |
				grep -v "^$criterion_dir/report/index.html$" |
				sed 's/^/- Individual: /' || true
			echo ""
			echo "## Benchmark Results Summary"
		} >benchmark-summary.txt

		write_criterion_details "$criterion_dir"

		if [[ "$regression_found" == "true" ]]; then
			{
				echo ""
				echo "## Regression Analysis"
				echo "⚠️ REGRESSION DETECTED"
				echo "Threshold: $REGRESSION_THRESHOLD"
			} >>benchmark-summary.txt
		else
			{
				echo ""
				echo "## Regression Analysis"
				echo "✅ No significant regressions"
				echo "Threshold: $REGRESSION_THRESHOLD"
			} >>benchmark-summary.txt
		fi
	fi

	if [[ "$regression_found" == "true" ]]; then
		warn "⚠ Performance regressions detected in detailed analysis"
		detail "Check the HTML report for details: target/criterion/report/index.html"
		return 1
	fi

	note "✓ No significant regressions detected"
}

open_report() {
	[[ "$OPEN_REPORT" == "true" ]] || return 0
	local report="target/criterion/report/index.html"
	[[ -f "$report" ]] || return 0

	info "Opening benchmark report..."

	if have_cmd xdg-open; then
		xdg-open "$report" 2>/dev/null &
	elif have_cmd open; then
		open "$report" 2>/dev/null &
	elif have_cmd start; then
		start "$report" 2>/dev/null &
	else
		info "Could not auto-open report. Please open: $report"
	fi
}

show_results_location() {
	[[ "$SHOW_OUTPUT" == "true" ]] || return 0

	printf '%b\n' "${BLUE}${BOLD}Results Location:${NC}"

	[[ -f "target/criterion/report/index.html" ]] &&
		note "✓ HTML Report: target/criterion/report/index.html"
	[[ -n "$SAVE_BASELINE" && -d "baselines/performance/$SAVE_BASELINE" ]] &&
		note "✓ Saved baseline: baselines/performance/$SAVE_BASELINE/"
	[[ -f "benchmark-summary.txt" ]] &&
		note "✓ Summary: benchmark-summary.txt"

	printf '%b\n' "${BLUE}✓ Test package used: $TEST_PACKAGE_PATH${NC}"

	echo ""
	printf '%b\n' "${YELLOW}Tips:${NC}"
	detail "  • Use --open to automatically open the HTML report"
	detail "  • Use --save <name> to create a baseline for future comparisons"
	detail "  • Use --load <name> to compare against a saved baseline"
	detail "  • Use --test-package <path> to benchmark specific test data"
	echo ""
}

# Fallback summary when criterion produced no data. Header comes from
# write_summary_header so both writers stay in step.
create_basic_summary() {
	if [[ -f "benchmark-summary.txt" ]]; then
		return 0
	fi

	{
		write_summary_header
		echo ""
		echo "## Analysis Performance"
	} >benchmark-summary.txt

	local analysis_time baseline_time
	if [[ -n "$SAVE_BASELINE" && -f "baselines/performance/$SAVE_BASELINE/analysis_time.txt" ]]; then
		analysis_time="$(cat "baselines/performance/$SAVE_BASELINE/analysis_time.txt")"
		{
			echo "Analysis Time: ${analysis_time}s"
		} >>benchmark-summary.txt
	fi

	if [[ "$COMPARE_MODE" == "true" && -f "baselines/performance/$LOAD_BASELINE/analysis_time.txt" ]]; then
		baseline_time="$(cat "baselines/performance/$LOAD_BASELINE/analysis_time.txt")"
		{
			echo "Baseline Time: ${baseline_time}s"
			echo "Threshold: $REGRESSION_THRESHOLD"
		} >>benchmark-summary.txt
	fi

	{
		echo ""
		echo "## Environment"
		echo "Rust Version: $(rustc --version 2>/dev/null || echo 'Unknown')"
		echo "Host: $(rustc -vV 2>/dev/null | awk '/^host:/ { print $2 }' || echo 'Unknown')"
	} >>benchmark-summary.txt
}

# Main execution
main() {
	print_header
	check_prerequisites
	clean_build
	build_rustowl
	run_benchmarks

	local exit_code=0
	analyze_regressions || exit_code=1

	# Ensure we have a summary file for CI
	create_basic_summary

	open_report
	show_results_location

	if [[ "$SHOW_OUTPUT" == "true" ]]; then
		if [[ $exit_code -eq 0 ]]; then
			printf '%b\n' "${GREEN}${BOLD}✓ Benchmark completed successfully!${NC}"
		else
			printf '%b\n' "${RED}${BOLD}⚠ Benchmark completed with performance regressions detected${NC}"
		fi
	fi

	exit $exit_code
}

# Run main function
main "$@"
