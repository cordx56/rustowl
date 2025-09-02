#!/usr/bin/env bash

set -euo pipefail

# RustOwl Binary Size Monitoring Script
# Tracks and validates binary size metrics

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

# Configuration
SIZE_BASELINE_FILE="baselines/size_baseline.txt"
SIZE_THRESHOLD_PCT=10 # Warn if binary size increases by more than 10%

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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
	echo "    -h, --help     Show this help message"
	echo "    -t, --threshold <PCT>  Set size increase threshold (default: ${SIZE_THRESHOLD_PCT}%)"
	echo "    -v, --verbose  Show verbose output"
	echo ""
	echo "EXAMPLES:"
	echo "    $0                    # Check current binary sizes"
	echo "    $0 baseline           # Create baseline from current build"
	echo "    $0 compare            # Compare with baseline"
	echo "    $0 -t 15 compare      # Compare with 15% threshold"
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
	if command -v numfmt &>/dev/null; then
		numfmt --to=iec-i --suffix=B "$size"
	else
		# Fallback formatting
		if [ "$size" -ge 1048576 ]; then
			echo "$(($size / 1048576))MB"
		elif [ "$size" -ge 1024 ]; then
			echo "$(($size / 1024))KB"
		else
			echo "${size}B"
		fi
	fi
}

# Build binaries if they don't exist
ensure_binaries_built() {
	local binaries=(
		"target/release/rustowl"
		"target/release/rustowlc"
	)

	local need_build=false
	for binary in "${binaries[@]}"; do
		if [ ! -f "$binary" ]; then
			need_build=true
			break
		fi
	done

	if $need_build; then
		log_info "Building release binaries..."
		if ! ./scripts/build/toolchain cargo build --release; then
			log_error "Failed to build release binaries"
			exit 1
		fi
	fi
}

# Check current binary sizes
check_sizes() {
	log_info "Checking binary sizes..."

	ensure_binaries_built

	local binaries=(
		"target/release/rustowl"
		"target/release/rustowlc"
	)

	echo ""
	printf "%-20s %10s %15s\n" "Binary" "Size" "Formatted"
	printf "%-20s %10s %15s\n" "------" "----" "---------"

	for binary in "${binaries[@]}"; do
		local size
		size=$(get_binary_size "$binary")
		local formatted
		formatted=$(format_size "$size")
		local name
		name=$(basename "$binary")

		printf "%-20s %10d %15s\n" "$name" "$size" "$formatted"
	done
	echo ""
}

# Create size baseline
create_baseline() {
	log_info "Creating size baseline..."

	ensure_binaries_built

	local binaries=(
		"target/release/rustowl"
		"target/release/rustowlc"
	)

	# Create target directory if it doesn't exist
	mkdir -p "$(dirname "$SIZE_BASELINE_FILE")"

	# Write baseline
	{
		echo "# RustOwl Binary Size Baseline"
		echo "# Generated on $(date)"
		echo "# Format: binary_name:size_in_bytes"
		for binary in "${binaries[@]}"; do
			local size
			size=$(get_binary_size "$binary")
			local name
			name=$(basename "$binary")
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

	local binaries=(
		"target/release/rustowl"
		"target/release/rustowlc"
	)

	local any_issues=false

	echo ""
	printf "%-20s %12s %12s %10s %8s\n" "Binary" "Baseline" "Current" "Diff" "Change"
	printf "%-20s %12s %12s %10s %8s\n" "------" "--------" "-------" "----" "------"

	for binary in "${binaries[@]}"; do
		local name
		name=$(basename "$binary")

		# Get baseline size
		local baseline_size
		baseline_size=$(grep "^$name:" "$SIZE_BASELINE_FILE" | cut -d: -f2 || echo "0")

		if [ "$baseline_size" = "0" ]; then
			log_warning "No baseline found for $name"
			continue
		fi

		# Get current size
		local current_size
		current_size=$(get_binary_size "$binary")

		if [ "$current_size" = "0" ]; then
			log_error "Binary $name not found"
			any_issues=true
			continue
		fi

		# Calculate difference
		local diff=$((current_size - baseline_size))
		local pct_change=0

		if [ "$baseline_size" -gt 0 ]; then
			pct_change=$(echo "scale=1; $diff * 100 / $baseline_size" | bc 2>/dev/null || echo "0")
		fi

		# Format for display
		local baseline_fmt current_fmt diff_fmt
		baseline_fmt=$(format_size "$baseline_size")
		current_fmt=$(format_size "$current_size")

		if [ "$diff" -gt 0 ]; then
			diff_fmt="+$(format_size "$diff")"
		elif [ "$diff" -lt 0 ]; then
			diff_fmt="-$(format_size $((-diff)))"
		else
			diff_fmt="0B"
		fi

		printf "%-20s %12s %12s %10s %7s%%\n" "$name" "$baseline_fmt" "$current_fmt" "$diff_fmt" "$pct_change"

		# Check threshold
		local abs_pct_change
		abs_pct_change=$(echo "$pct_change" | tr -d '-')

		if (($(echo "$abs_pct_change > $SIZE_THRESHOLD_PCT" | bc -l))); then
			if [ "$diff" -gt 0 ]; then
				log_warning "$name size increased by $pct_change% (threshold: ${SIZE_THRESHOLD_PCT}%)"
			else
				log_info "$name size decreased by $pct_change%"
			fi
			any_issues=true
		fi
	done

	echo ""

	if $any_issues; then
		log_warning "Some binaries exceeded size thresholds"
		exit 1
	else
		log_success "All binary sizes within acceptable ranges"
	fi
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
	local verbose=false

	# Parse arguments
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
			shift 2
			;;
		-v | --verbose)
			verbose=true
			shift
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

	# Ensure bc is available for calculations
	if ! command -v bc &>/dev/null; then
		log_error "bc (basic calculator) is required but not installed"
		log_info "Install with: apt-get install bc"
		exit 1
	fi

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
