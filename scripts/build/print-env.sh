#!/bin/sh -e

TOOLCHAIN_CHANNEL="${TOOLCHAIN_CHANNEL:-"$1"}"

if [ -z "$TOOLCHAIN_CHANNEL" ]; then
  echo "Usage: $0 <rust-version>"
  echo "Example: $0 1.89.0"
  exit 1
fi

# print host-tuple
host_tuple() {
    if [ -z "$TOOLCHAIN_OS" ]; then
        # Get OS
        case "$(uname -s)" in
            Linux)
                TOOLCHAIN_OS="unknown-linux-gnu"
                ;;
            Darwin)
                TOOLCHAIN_OS="apple-darwin"
                ;;
            CYGWIN*|MINGW32*|MSYS*|MINGW*)
                TOOLCHAIN_OS="pc-windows-msvc"
                ;;
            *)
                echo "Unsupported OS: $(uname -s)" >&2
                exit 1
                ;;
        esac
    fi

    if [ -z "$TOOLCHAIN_ARCH" ]; then
        # Get architecture
        case "$(uname -m)" in
            arm64|aarch64)
                TOOLCHAIN_ARCH="aarch64"
                ;;
            x86_64|amd64)
                TOOLCHAIN_ARCH="x86_64"
                ;;
            *)
                echo "Unsupported architecture: $(uname -m)" >&2
                exit 1
                ;;
        esac
    fi

    echo "$TOOLCHAIN_ARCH-$TOOLCHAIN_OS"
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

    # Lto fix...
    case "$(host_tuple)" in
        *-pc-windows-msvc)
            echo "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=lld-link.exe"
            echo "RUSTFLAGS=-Clinker=lld-link"
            echo "CC=clang-cl"
            echo "CXX=clang-cl"
            echo "CFLAGS=/clang:-flto=fat /clang:-fuse-ld=lld-link"
            echo "CXXFLAGS=/clang:-flto=fat /clang:-fuse-ld=lld-link"
            echo "AR=llvm-lib"
            ;;
    esac
}

print_env
