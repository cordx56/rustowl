#!/bin/sh -e

if [ $# -ne 1 ]; then
  echo "Usage: $0 <rust-version>"
  echo "Example: $0 1.89.0"
  exit 1
fi

TOOLCHAIN_CHANNEL="$1"

# print host-tuple
host_tuple() {
    # Get OS
    case "$(uname -s)" in
        Linux)
            os="unknown-linux-gnu"
            ;;
        Darwin)
            os="apple-darwin"
            ;;
        CYGWIN*|MINGW32*|MSYS*|MINGW*)
            os="pc-windows-msvc"
            ;;
        *)
            echo "Unsupported OS: $(uname -s)" >&2
            exit 1
            ;;
    esac

    # Get architecture
    case "$(uname -m)" in
        arm64|aarch64)
            arch="aarch64"
            ;;
        x86_64|amd64)
            arch="x86_64"
            ;;
        *)
            echo "Unsupported architecture: $(uname -m)" >&2
            exit 1
            ;;
    esac

    echo "$arch-$os"
}

print_toolchain() {
    echo "${TOOLCHAIN_CHANNEL}-$(host_tuple)"
}


DIST_BASE="https://static.rust-lang.org/dist"
check_component() {
    component="$1"
    url="${DIST_BASE}/${component}-$(print_toolchain).tar.gz"

    component_sub="$(echo "${component}" | sed s/-/_/g)"
    echo "RUSTOWL_COMPONENT_${component_sub}_URL=${url}"
    echo "RUSTOWL_COMPONENT_${component_sub}_HASH=$(curl "$url.sha256" 2>/dev/null | awk '{ print $1 }')"
}

print_env() {
    echo "TOOLCHAIN_CHANNEL=${TOOLCHAIN_CHANNEL}"
    echo "RUSTOWL_TOOLCHAIN=$(print_toolchain)"
    echo "HOST_TUPLE=$(host_tuple)"
    check_component "rustc"
    check_component "rust-std"
    check_component "cargo"
    check_component "rustc-dev"
    check_component "llvm-tools"
}

print_env
