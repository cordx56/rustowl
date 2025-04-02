#!/bin/bash -x

VERSION="0.2.0"

SED=$(which gsed || echo "sed")
SED_SUBSTITUTE="[0-9]\+\.[0-9]\+\.[0-9]\+/$VERSION"

# RustOwl
$SED -i -e "3 s/$SED_SUBSTITUTE/g" rustowl/install.sh
$SED -i -e "3 s/$SED_SUBSTITUTE/g" rustowl/Cargo.toml

# VSCode extension
$SED -i -e "5 s/$SED_SUBSTITUTE/g" vscode/package.json
