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


print_env() {
    echo "TOOLCHAIN_CHANNEL=${TOOLCHAIN_CHANNEL}"
    toolchain="$(print_toolchain)"
    echo "RUSTOWL_TOOLCHAIN=$toolchain"
    echo "HOST_TUPLE=$(host_tuple)"
    sysroot="${SYSROOT:-"$HOME/.rustowl/sysroot/$toolchain"}"
    echo "SYSROOT=$sysroot"
    echo "PATH=$sysroot/bin:$PATH"
    echo "RUSTC_BOOTSTRAP=rustowlc"
}

print_env
