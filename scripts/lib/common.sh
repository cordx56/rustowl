# shellcheck shell=sh
# Shared helpers for the RustOwl development scripts.
#
# POSIX sh on purpose: scripts/toolchain sources this and runs under dash /
# busybox-ash in CI. So: no [[ ]], no arrays, no `echo -e` — colours go through
# printf, which behaves identically in bash and sh.
#
# This file must have NO side effects at source time (no cd, no export) — that is
# what makes it safe to source into scripts/toolchain, which sets up its own
# environment before doing anything.
#
# scripts/installer deliberately does NOT source this file. It is piped straight
# into `sh` from a URL (see docs/installation.md), so it has to stay
# dependency-free and keeps its own copy of print_host_tuple.

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
# Deliberately plain assignments: scripts that want them unset can do
# `RED= GREEN=` after sourcing. Each one is exported for consumers, so not every
# script that sources this file uses every colour.
# shellcheck disable=SC2034
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

log_info() { printf '%b\n' "${BLUE}[INFO]${NC} $1"; }
log_success() { printf '%b\n' "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { printf '%b\n' "${YELLOW}[WARNING]${NC} $1"; }
log_error() { printf '%b\n' "${RED}[ERROR]${NC} $1"; }

# ---------------------------------------------------------------------------
# Environment
# ---------------------------------------------------------------------------

# NixOS/nix-ld runs dynamically linked toolchain binaries without an ELF
# interpreter, so the system shared libraries (libz in particular) have to be
# exposed to both the loader and the linker.
setup_nix_ld_paths() {
	if [ -n "${NIX_LD_LIBRARY_PATH:-}" ]; then
		export LD_LIBRARY_PATH="${LD_LIBRARY_PATH:+$LD_LIBRARY_PATH:}$NIX_LD_LIBRARY_PATH"
		export LIBRARY_PATH="${LIBRARY_PATH:+$LIBRARY_PATH:}$NIX_LD_LIBRARY_PATH"
	fi
}

# Return 0 if $1 names an executable on PATH.
have_cmd() {
	command -v "$1" >/dev/null 2>&1
}

# Print an error and exit 1 unless $1 is on PATH. $2 is an optional hint.
require_cmd() {
	if ! have_cmd "$1"; then
		log_error "$1 is required but was not found."
		if [ -n "${2:-}" ]; then
			log_info "$2"
		fi
		exit 1
	fi
}

# ---------------------------------------------------------------------------
# Host tuple
# ---------------------------------------------------------------------------

# Print "$TOOLCHAIN_ARCH-$TOOLCHAIN_OS", deriving either half from uname when
# it is not already exported. Exits 1 on an OS or arch we cannot name.
#
# TOOLCHAIN_OS / TOOLCHAIN_ARCH are honoured as inputs rather than derived
# unconditionally, because callers may pin them (cross-compilation, tests).
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

# ---------------------------------------------------------------------------
# Rust toolchain
# ---------------------------------------------------------------------------

# Print the active rustc version as X.Y[.Z], or nothing if rustc is absent or
# its version cannot be parsed. Strips any -nightly / -beta suffix.
rust_version() {
	if ! have_cmd rustc; then
		return 0
	fi
	rustc --version 2>/dev/null |
		grep -oE '[0-9]+\.[0-9]+(\.[0-9]+)?' |
		head -1
}

# Return 0 when version $1 is at least minimum $2. An empty version is never
# at least anything, so callers can treat a missing rustc as "cannot verify".
#
# Compares with `sort -V -C`, which orders 1.100 above 1.87. A hand-rolled
# major/minor comparison gets that wrong (9 < 87), which is why this is shared
# instead of reimplemented per script.
version_at_least() {
	have="$1"
	want="$2"
	[ -n "$have" ] || return 1
	printf '%s\n%s\n' "$want" "$have" | sort -V -C 2>/dev/null
}

# Log the outcome of a rustc version gate and return 0/1. Never exits, so the
# caller keeps ownership of its own failure accounting.
check_rust_version() {
	# No `local` here: it is not in POSIX, and this file has to survive dash and
	# busybox ash. Underscore-prefixed names keep it from clobbering callers.
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

# ---------------------------------------------------------------------------
# Percentage arithmetic
# ---------------------------------------------------------------------------

# Render a count of tenths as a signed percentage: 104 with "+" is +10.4,
# 300 with "-" is -30.0, 0 with "" is 0.0. Integer-only, so the scripts that
# need this do not depend on bc.
format_tenths() {
	printf '%s%d.%d' "$2" "$(($1 / 10))" "$(($1 % 10))"
}

# Signed change from size $1 to size $2, in tenths of a percent: growth prints
# unsigned (104), a shrink prints negative (-300). Truncates toward zero, and a
# non-positive baseline yields 0 rather than dividing by zero.
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
