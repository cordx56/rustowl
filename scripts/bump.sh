#!/bin/bash

# Script to update version numbers in multiple files and create a git tag
# Usage: ./bump.sh v0.3.1

# Ensure a version argument is provided
if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>"
  echo "Example: $0 v0.3.1"
  exit 1
fi

VERSION=$1
VERSION_WITHOUT_V="${VERSION#v}"

echo "Updating to version: $VERSION"

# Check if version contains alpha, beta, rc, dev, or other pre-release identifiers
if echo "$VERSION_WITHOUT_V" | grep -q -E 'alpha|beta|rc|dev|pre|snapshot'; then
  IS_PRERELEASE=true
  echo "Pre-release version detected ($VERSION_WITHOUT_V). PKGBUILD will not be updated."
else
  IS_PRERELEASE=false
  echo "Stable version detected ($VERSION_WITHOUT_V)."
fi

# 1. Update Cargo.toml in root directory (only the first version field)
if [ -f Cargo.toml ]; then
  echo "Updating Cargo.toml..."
  # Use sed to replace only the first occurrence of the version line
  sed -i '0,/^version = .*/{s/^version = .*/version = "'$VERSION_WITHOUT_V'"/}' Cargo.toml
else
  echo "Error: Cargo.toml not found in current directory"
  exit 1
fi

# 2. Update vscode/package.json
if [ -f vscode/package.json ]; then
  echo "Updating vscode/package.json..."
  # Use sed to replace the "version": "x.x.x" line
  sed -i "s/\"version\": \".*\"/\"version\": \"$VERSION_WITHOUT_V\"/" vscode/package.json
else
  echo "Warning: vscode/package.json not found"
fi

# 3. Update aur/PKGBUILD only for stable releases
if [ "$IS_PRERELEASE" = false ] && [ -f aur/PKGBUILD ]; then
  echo "Updating aur/PKGBUILD..."
  # Use sed to replace the pkgver line
  sed -i "s/^pkgver=.*/pkgver=$VERSION_WITHOUT_V/" aur/PKGBUILD
elif [ -f aur/PKGBUILD ]; then
  echo "Skipping aur/PKGBUILD update for pre-release version"
else
  echo "Warning: aur/PKGBUILD not found"
fi

# 4. Create a git tag
echo "Creating git tag: $VERSION"
git tag $VERSION

echo "Version bump complete. Changes have been made to the files."
echo "Remember to commit your changes before pushing the tag."
echo "To push the tag: git push origin $VERSION"
