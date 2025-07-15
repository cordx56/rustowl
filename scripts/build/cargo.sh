#!/bin/sh -e

script_dir="$(cd $(dirname "$0"); pwd)"

export $(/bin/sh "$script_dir/print-env.sh" "$(cat "${script_dir}/toolchain")")

export SYSROOT="${SYSROOT:-"$HOME/.cache/rustowl/sysroot/${RUSTOWL_TOOLCHAIN}"}"
export RUSTC="${RUSTC:-"${SYSROOT}/bin/rustc"}"
export CARGO="${CARGO:-"${SYSROOT}/bin/cargo"}"

# install_toolchain <sysroot-path>
install_toolchain() {
    mkdir -p "$1"

    for url in $(env | grep "RUSTOWL_COMPONENT_.*_URL" | cut -d= -f2); do
        temp="$(mktemp -d)"
        curl "$url" | tar xzf - -C "$temp"
        /bin/sh "$(find "$temp" -type f -maxdepth 2 | grep "install.sh")" --destdir="$1" --prefix=
        rm -rf "$temp"
    done
}

build() {
    cd "${script_dir}/../../"

    RUSTC_BOOTSTRAP=rustowlc $CARGO "$@"
}

#
# main
#
if [ ! -d "${SYSROOT}" ]; then
    install_toolchain "${SYSROOT}"
fi

build "$@"
