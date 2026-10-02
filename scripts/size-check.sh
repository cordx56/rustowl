#!/usr/bin/env bash

set -euo pipefail

# RustOwl Binary Size Monitoring Script
# Tracks and validates binary size metrics

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=scripts/lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

cd "$REPO_ROOT"

setup_nix_ld_paths

# Configuration
SIZE_BASELINE_FILE="baselines/size_baseline.txt"
SIZE_THRESHOLD_PCT=10 # Warn if a binary grows by more than this many percent

# The binaries we track. Used by every command below.
BINARIES=(
	"target/release/rustowl"
	"target/release/rustowlc"
)

show_help() {
	echo "RustOwl Binary Size Monitoring"
	echo ""
	echo "USAGE:"
	echo "    $0 [OPTIONS] [COMMAND]"
	echo ""
	echo "COMMANDS:"
	echo "    check          Check current binary sizes (default)"
	echo "    baseline       Create/update size baseline"
	echo "    compare        Compare current sizes with baseline"
	echo "    clean          Remove baseline file"
	echo ""
	echo "OPTIONS:"
	echo "    -h, --help            Show this help message"
	echo "    -t, --threshold <PCT> Fail if a binary grows by more than PCT percent"
	echo "                         (default: ${SIZE_THRESHOLD_PCT}; whole numbers only)"
	echo ""
	echo "EXAMPLES:"
	echo "    $0                    # Check current binary sizes"
	echo "    $0 baseline           # Create baseline from current build"
	echo "    $0 compare            # Compare with baseline"
	echo "    $0 -t 15 compare      # Compare with 15% threshold"
	echo ""
	echo "Only growth is treated as a regression: a binary that got smaller never"
	echo "fails the comparison, whatever the threshold."
}

# Get binary size in bytes
get_binary_size() {
	local binary_path="$1"
	if [ -f "$binary_path" ]; then
		stat --format="%s" "$binary_path" 2>/dev/null || stat -f%z "$binary_path" 2>/dev/null || echo "0"
	else
		echo "0"
	fi
}

# Format size for human reading
format_size() {
	local size="$1"
	if have_cmd numfmt; then
		numfmt --to=iec-i --suffix=B "$size"
	else
		# Fallback formatting
		if [ "$size" -ge 1048576 ]; then
			echo "$((size / 1048576))MB"
		elif [ "$size" -ge 1024 ]; then
			echo "$((size / 1024))KB"
		else
			echo "${size}B"
		fi
	fi
}

# Reject thresholds that are not whole percent. The arithmetic below is integer
# tenths, so a fractional threshold like 2.5 would be a $(( )) syntax error
# rather than a usable value.
validate_threshold() {
	case "$1" in
	'' | *[!0-9]*)
		log_error "Threshold must be a whole number of percent, got: $1"
		log_info "Example: $0 -t 10 compare"
		exit 1
		;;
	esac
	return 0
}

# Build binaries if they don't exist
ensure_binaries_built() {
	local binary
	for binary in "${BINARIES[@]}"; do
		if [ ! -f "$binary" ]; then
			log_info "Building release binaries..."
			if ! ./scripts/toolchain cargo build --release; then
				log_error "Failed to build release binaries"
				exit 1
			fi
			return 0
		fi
	done
}

# Check current binary sizes
check_sizes() {
	log_info "Checking binary sizes..."

	ensure_binaries_built

	local binary size formatted name
	echo ""
	printf "%-20s %10s %15s\n" "Binary" "Size" "Formatted"
	printf "%-20s %10s %15s\n" "------" "----" "---------"

	for binary in "${BINARIES[@]}"; do
		size=$(get_binary_size "$binary")
		formatted=$(format_size "$size")
		name="${binary##*/}"

		printf "%-20s %10d %15s\n" "$name" "$size" "$formatted"
	done
	echo ""
}

# Create size baseline
create_baseline() {
	log_info "Creating size baseline..."

	ensure_binaries_built

	# Create target directory if it doesn't exist
	mkdir -p "$(dirname "$SIZE_BASELINE_FILE")"

	local binary size name
	{
		echo "# RustOwl Binary Size Baseline"
		echo "# Generated on $(date)"
		echo "# Format: binary_name:size_in_bytes"
		for binary in "${BINARIES[@]}"; do
			size=$(get_binary_size "$binary")
			name="${binary##*/}"
			echo "$name:$size"
		done
	} >"$SIZE_BASELINE_FILE"

	log_success "Baseline created at $SIZE_BASELINE_FILE"

	# Show what was recorded
	echo ""
	log_info "Baseline contents:"
	check_sizes
}

# Compare with baseline
compare_with_baseline() {
	if [ ! -f "$SIZE_BASELINE_FILE" ]; then
		log_error "No baseline file found at $SIZE_BASELINE_FILE"
		log_info "Run '$0 baseline' to create one"
		exit 1
	fi

	log_info "Comparing with baseline (threshold: ${SIZE_THRESHOLD_PCT}%)..."

	ensure_binaries_built

	local threshold_tenths=$((SIZE_THRESHOLD_PCT * 10))
	local any_issues=false
	local binary name baseline_size current_size diff sign
	local pct_tenths pct_change pct_magnitude baseline_fmt current_fmt diff_fmt

	echo ""
	printf "%-20s %12s %12s %10s %8s\n" "Binary" "Baseline" "Current" "Diff" "Change"
	printf "%-20s %12s %12s %10s %8s\n" "------" "--------" "-------" "----" "------"

	for binary in "${BINARIES[@]}"; do
		name="${binary##*/}"

		# Get baseline size
		baseline_size=$(grep "^$name:" "$SIZE_BASELINE_FILE" | cut -d: -f2 || echo "0")

		if [ "$baseline_size" = "0" ]; then
			log_warning "No baseline found for $name"
			continue
		fi

		# Get current size
		current_size=$(get_binary_size "$binary")

		if [ "$current_size" = "0" ]; then
			log_error "Binary $name not found"
			any_issues=true
			continue
		fi

		diff=$((current_size - baseline_size))

		# Signed tenths of a percent, then the sign and magnitude split out: the
		# comparison only cares about growth, and the report reads better as
		# "decreased by 30.0%" than "decreased by -30.0%".
		pct_tenths=$(pct_change_tenths "$baseline_size" "$current_size")
		if [ "${pct_tenths#-}" != "$pct_tenths" ]; then
			sign="-"
			abs_diff="${pct_tenths#-}"
		else
			sign="+"
			abs_diff="$pct_tenths"
		fi

		pct_change="$(format_tenths "$abs_diff" "$sign")"
		# "increased by 10.4%" reads better than "increased by +10.4%", so the
		# prose uses the magnitude and the sign only in the table above.
		pct_magnitude="$(format_tenths "$abs_diff" "")"

		# Format for display
		baseline_fmt=$(format_size "$baseline_size")
		current_fmt=$(format_size "$current_size")

		if [ "$diff" -gt 0 ]; then
			diff_fmt="+$(format_size "$diff")"
		elif [ "$diff" -lt 0 ]; then
			diff_fmt="-$(format_size "$((-diff))")"
		else
			diff_fmt="0B"
		fi

		printf "%-20s %12s %12s %10s %7s%%\n" "$name" "$baseline_fmt" "$current_fmt" "$diff_fmt" "$pct_change"

		# Only growth can be a regression. Comparing the magnitude meant a 30%
		# shrink used to trip this branch and fail the run.
		if [ "$diff" -gt 0 ] && [ "$pct_tenths" -gt "$threshold_tenths" ]; then
			log_warning "$name size increased by ${pct_magnitude}% (threshold: ${SIZE_THRESHOLD_PCT}%)"
			any_issues=true
		elif [ "$diff" -lt 0 ]; then
			log_info "$name size decreased by ${pct_magnitude}%"
		fi
	done

	echo ""

	if $any_issues; then
		log_warning "Some binaries exceeded size thresholds"
		exit 1
	fi

	log_success "All binary sizes within acceptable ranges"
}

# Clean baseline
clean_baseline() {
	if [ -f "$SIZE_BASELINE_FILE" ]; then
		rm "$SIZE_BASELINE_FILE"
		log_success "Baseline file removed"
	else
		log_info "No baseline file to remove"
	fi
}

main() {
	local command="check"

	while [[ $# -gt 0 ]]; do
		case $1 in
		-h | --help)
			show_help
			exit 0
			;;
		-t | --threshold)
			if [[ $# -lt 2 ]]; then
				log_error "Option --threshold requires a value"
				exit 1
			fi
			SIZE_THRESHOLD_PCT="$2"
			validate_threshold "$SIZE_THRESHOLD_PCT"
			shift 2
			;;
		check | baseline | compare | clean)
			command="$1"
			shift
			;;
		*)
			log_error "Unknown option: $1"
			show_help
			exit 1
			;;
		esac
	done

	case $command in
	check)
		check_sizes
		;;
	baseline)
		create_baseline
		;;
	compare)
		compare_with_baseline
		;;
	clean)
		clean_baseline
		;;
	*)
		log_error "Unknown command: $command"
		show_help
		exit 1
		;;
	esac
}

main "$@"
