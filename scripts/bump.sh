#!/usr/bin/env bash

# Update version numbers across the repo and create the matching git tag.
# Usage: ./scripts/bump.sh v0.3.1

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=scripts/lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

# Every path below is relative to the repo root, so resolve it explicitly rather
# than depending on where bump.sh was invoked from.
cd "$REPO_ROOT"

usage() {
	echo "Usage: $0 <version>"
	echo "Example: $0 v0.3.1"
	echo ""
	echo "Rewrites the version in Cargo.toml, vscode/package.json and the AUR"
	echo "PKGBUILDs, then creates a git tag for it."
	echo ""
	echo "  <version>   A release version such as v0.3.1 or v1.0.0-rc.1."
	echo "              A leading 'v' is optional. Pre-releases (containing"
	echo "              alpha, beta, rc, dev, pre or snapshot) leave the AUR"
	echo "              PKGBUILDs untouched, since those are not published."
	echo ""
	echo "  -h, --help  Show this help"
}

# In-place editing differs between the two seds: BSD sed (macOS) requires a
# suffix argument after -i, GNU sed rejects one. gsed is Homebrew's GNU sed.
if have_cmd gsed; then
	SED_CMD=(gsed -i)
elif [[ "$(uname -s)" == "Darwin" ]]; then
	SED_CMD=(sed -i '')
else
	SED_CMD=(sed -i)
fi

case "${1:-}" in
-h | --help)
	usage
	exit 0
	;;
esac

if [[ $# -ne 1 ]]; then
	usage
	exit 1
fi

VERSION="$1"
VERSION_WITHOUT_V="${VERSION#v}"

# Refuse anything that is not a release version *before* touching a single file.
# Without this, `bump.sh --help` rewrote every version string in the repo to
# "--help", because the only guard was an argument count.
if [[ ! "$VERSION_WITHOUT_V" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
	log_error "'$VERSION' is not a valid version."
	log_info "Expected something like v0.3.1 or v1.0.0-rc.1 (got: $0 --help for usage)"
	exit 1
fi

log_info "Updating to version: $VERSION"

# Pre-releases are not published to the AUR, so they leave those files alone.
if [[ "$VERSION_WITHOUT_V" =~ (alpha|beta|rc|dev|pre|snapshot) ]]; then
	IS_PRERELEASE=true
	log_info "Pre-release version detected ($VERSION_WITHOUT_V). aur/PKGBUILD will not be updated."
else
	IS_PRERELEASE=false
	log_info "Stable version detected ($VERSION_WITHOUT_V)."
fi

if [[ ! -f Cargo.toml ]]; then
	log_error "Cargo.toml not found in $REPO_ROOT"
	exit 1
fi

# Rewrite the first line matching <line_regex> in <file> with <replacement>.
# This is the one place version strings are edited; the callers below only say
# which file holds which form.
replace_version_line() {
	local file="$1"
	local line_regex="$2"
	local replacement="$3"

	if [[ ! -f "$file" ]]; then
		log_warning "$file not found, skipping"
		return 0
	fi

	"${SED_CMD[@]}" "0,/$line_regex/{s/$line_regex/$replacement/}" "$file"
	log_info "Updated $file"
}

# aur/PKGBUILD and aur/PKGBUILD-BIN carry the same pkgver= line.
update_pkgver() {
	local file="aur/$1"

	if [[ $IS_PRERELEASE == true ]]; then
		if [[ -f "$file" ]]; then
			log_info "Skipping $file update for pre-release version"
		fi
		return 0
	fi

	replace_version_line "$file" '^pkgver=.*' "pkgver=$VERSION_WITHOUT_V"
}

# Eask and rustowl.el carry the version for the Emacs package. Like the AUR
# files, only updated for stable releases.
update_emacs_version() {
	if [[ $IS_PRERELEASE == true ]]; then
		if [[ -f Eask || -f rustowl.el ]]; then
			log_info "Skipping Eask and rustowl.el update for pre-release version"
		fi
		return 0
	fi

	# Eask's version is an indented quoted string, so the leading whitespace is
	# captured in the pattern and restored by the \1 in the replacement.
	local eask_line='\1"'"$VERSION_WITHOUT_V"'"'
	replace_version_line Eask '^\([[:space:]]*\)"[0-9][^"]*"$' "$eask_line"
	replace_version_line rustowl.el '^;; Version: .*' ";; Version: $VERSION_WITHOUT_V"
}

# Check the tag before editing anything, so a refusal cannot leave the files
# rewritten without a matching tag.
if git rev-parse --verify --quiet "refs/tags/$VERSION" >/dev/null; then
	log_error "Tag $VERSION already exists; delete it first or pick another version"
	exit 1
fi

# Only the first `version = ` line is replaced, so a workspace-level version
# above the package table is left intact.
replace_version_line Cargo.toml '^version = .*' "version = \"$VERSION_WITHOUT_V\""
replace_version_line vscode/package.json '"version": ".*"' "\"version\": \"$VERSION_WITHOUT_V\""
update_pkgver PKGBUILD
update_pkgver PKGBUILD-BIN
update_emacs_version

log_info "Creating git tag: $VERSION"
git tag "$VERSION"

log_success "Version bump complete. Changes have been made to the files."
log_info "Remember to commit your changes before pushing the tag."
log_info "To push the tag: git push origin $VERSION"
