#!/bin/sh

VERSION="0.2.0"

install_rust() {
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -q -y --profile minimal
}

install_rustowl() {
    if ! which curl > /dev/null 2>&1; then
        echo "curl command not found; error"
        return
    fi
    TMP="$(mktemp -d)"
    cd "$TMP"
    curl -L "https://github.com/cordx56/rustowl/archive/refs/tags/v$VERSION.tar.gz" > rustowl.tar.gz
    tar -xvf rustowl.tar.gz
    cd rustowl-*/rustowl
    cargo install --path . --locked
    cd ../../
    rm -rf "$TMP"
}

install_rustowl
