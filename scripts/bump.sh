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

# 1. Update Cargo.toml in root directory
if [ -f Cargo.toml ]; then
	echo "Updating Cargo.toml..."
	# Use sed to replace the version line
	sed -i "s/^version = .*/version = \"$VERSION_WITHOUT_V\"/" Cargo.toml
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

# 3. Update aur/PKGBUILD
if [ -f aur/PKGBUILD ]; then
	echo "Updating aur/PKGBUILD..."
	# Use sed to replace the pkgver line
	sed -i "s/^pkgver=.*/pkgver=$VERSION_WITHOUT_V/" aur/PKGBUILD
else
	echo "Warning: aur/PKGBUILD not found"
fi

# 4. Create a git tag
echo "Creating git tag: $VERSION"
git tag $VERSION

echo "Version bump complete. Changes have been made to the files."
echo "Remember to commit your changes before pushing the tag."
echo "To push the tag: git push origin $VERSION"
