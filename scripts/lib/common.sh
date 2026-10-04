# shellcheck shell=sh
# Shared helpers for the RustOwl development scripts.
#
# POSIX sh is required: scripts/toolchain sources this under dash and
# busybox-ash in CI. No side effects at source time either.

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
# shellcheck disable=SC2034  # exported for consumers; unused in this file
BOLD='\033[1m'
NC='\033[0m' # No Color

# Diagnostics go to stderr so that helpers whose stdout is a value -- e.g.
# rustowl_release_binary, which echoes a path -- stay capturable via $(...).
log_info() { printf '%b\n' "${BLUE}[INFO]${NC} $1" >&2; }
log_success() { printf '%b\n' "${GREEN}[SUCCESS]${NC} $1" >&2; }
log_warning() { printf '%b\n' "${YELLOW}[WARNING]${NC} $1" >&2; }
log_error() { printf '%b\n' "${RED}[ERROR]${NC} $1" >&2; }

# nix-ld runs toolchain binaries without an ELF interpreter, so the system
# shared libraries have to be exposed to both loader and linker.
setup_nix_ld_paths() {
	if [ -n "${NIX_LD_LIBRARY_PATH:-}" ]; then
		export LD_LIBRARY_PATH="${LD_LIBRARY_PATH:+$LD_LIBRARY_PATH:}$NIX_LD_LIBRARY_PATH"
		export LIBRARY_PATH="${LIBRARY_PATH:+$LIBRARY_PATH:}$NIX_LD_LIBRARY_PATH"
	fi
}

have_cmd() {
	command -v "$1" >/dev/null 2>&1
}

require_cmd() {
	if ! have_cmd "$1"; then
		log_error "$1 is required but was not found."
		if [ -n "${2:-}" ]; then
			log_info "$2"
		fi
		exit 1
	fi
}

# Print "$TOOLCHAIN_ARCH-$TOOLCHAIN_OS". Either half is derived from uname only
# when not already set, since callers may pin them. Exits 1 if uname is unknown.
print_host_tuple() {
	if [ -z "${TOOLCHAIN_OS:-}" ]; then
		case "$(uname -s)" in
		Linux) TOOLCHAIN_OS="unknown-linux-gnu" ;;
		Darwin) TOOLCHAIN_OS="apple-darwin" ;;
		CYGWIN* | MINGW32* | MSYS* | MINGW*) TOOLCHAIN_OS="pc-windows-msvc" ;;
		*)
			echo "Unsupported OS: $(uname -s)" >&2
			exit 1
			;;
		esac
	fi

	if [ -z "${TOOLCHAIN_ARCH:-}" ]; then
		case "$(uname -m)" in
		arm64 | aarch64) TOOLCHAIN_ARCH="aarch64" ;;
		x86_64 | amd64) TOOLCHAIN_ARCH="x86_64" ;;
		*)
			echo "Unsupported architecture: $(uname -m)" >&2
			exit 1
			;;
		esac
	fi

	echo "$TOOLCHAIN_ARCH-$TOOLCHAIN_OS"
}

# Print the active rustc version as X.Y[.Z], or nothing if unavailable.
rust_version() {
	if ! have_cmd rustc; then
		return 0
	fi
	# The trailing `|| true` matters: under `pipefail` a grep that matches
	# nothing would otherwise abort the caller, but an unparsable version has
	# to read as empty so check_rust_version can report it properly.
	_raw="$(rustc --version 2>/dev/null || true)"
	printf '%s\n' "$_raw" |
		grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' |
		head -1 || true
}

# Return 0 when $1 is at least $2. An empty version is never at least anything,
# so a missing rustc reads as "cannot verify" rather than "too old".
#
# Each dot-separated component is compared as a number, so 1.100 is above 1.87.
# A naive string compare would not do that. `sort -V` would, but it is a GNU
# extension absent from macOS, and this library has to stay POSIX.
version_at_least() {
	# No `local` (not in POSIX). Underscores keep it clear of caller variables.
	_have="$1"
	_want="$2"
	[ -n "$_have" ] || return 1
	# "[.]" rather than "." because a bare dot is a regex matching any character.
	printf '%s %s\n' "$_have" "$_want" | awk '{
		nh = split($1, h, "[.]")
		nw = split($2, w, "[.]")
		for (i = 1; i <= 3; i++) {
			hv = (i <= nh) ? h[i] + 0 : 0
			wv = (i <= nw) ? w[i] + 0 : 0
			if (hv > wv) exit 0
			if (hv < wv) exit 1
		}
		exit 0
	}'
}

check_rust_version() {
	_min_version="$1"
	_current="$(rust_version)"

	if [ -z "$_current" ]; then
		log_error "Could not determine the Rust version (is rustc installed?)."
		log_info "Install Rust: https://rustup.rs/"
		return 1
	fi

	if version_at_least "$_current" "$_min_version"; then
		log_success "Rust $_current >= $_min_version (minimum required)"
		return 0
	fi

	log_error "Rust $_current < $_min_version (minimum required)"
	log_info "Update Rust with: rustup update"
	return 1
}

# Render tenths as a percentage: 104 "+" -> +10.4, 300 "-" -> -30.0. A minus
# sign already on $1 wins over $2, so "-30.0" never becomes "--30.0".
format_tenths() {
	_tenths="${1#-}"
	_sign="$2"
	# A negative value forces the minus: prepending to $2 would turn a "+"
	# sign into the nonsense "-+30.0".
	case $1 in
	-*) _sign="-" ;;
	esac
	printf '%s%d.%d' "$_sign" "$((_tenths / 10))" "$((_tenths % 10))"
}

# Signed change from $1 to $2 in tenths of a percent: 104 growth, -300 shrink.
# Truncates toward zero; a non-positive $1 yields 0 instead of dividing by zero.
pct_change_tenths() {
	_from="$1"
	_to="$2"
	_diff=$((_to - _from))

	[ "$_from" -gt 0 ] || {
		echo 0
		return 0
	}

	_tenths=$(((_diff < 0 ? -_diff : _diff) * 1000 / _from))
	if [ "$_diff" -lt 0 ]; then
		echo "-$_tenths"
	else
		echo "$_tenths"
	fi
}
