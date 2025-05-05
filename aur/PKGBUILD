# Maintainer: MuntasirSZN <muntasir.joypurhat@gmail.com>
# Maintainer: cordx56 <cordx56@cordx.cx>

pkgname=rustowl-bin
pkgver=0.3.0
pkgrel=1
pkgdesc='Visualize Ownership and Lifetimes in Rust'
url='https://github.com/cordx56/rustowl'
license=('MPL-2.0')
makedepends=('cargo' 'zig=0.13.0')
depends=()
conflicts=('rustowl-git')
arch=('any')
source=("https://github.com/cordx56/rustowl/archive/refs/tags/v${pkgver}.tar.gz")
sha256sums=('156f50d821ff4a1a3d02083fc558aadbdea2fbe21671426156389b5ccddacba7')

prepare() {
    cd rustowl-${pkgver}
    export RUSTC_BOOTSTRAP=1
    export RUSTUP_TOOLCHAIN=1.86.0
    rustup component add rust-src rustc-dev llvm-tools
    cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
    cargo install --locked cargo-zigbuild
}

build() {
    cd rustowl-${pkgver}
    export CARGO_TARGET_DIR=target
    export RUSTC_BOOTSTRAP=1
    export RUSTUP_TOOLCHAIN=1.86.0
    export RUSTOWL_RUNTIME_DIRS=/opt/rustowl
    cargo zigbuild --frozen --release --all-features --target $(rustc --print=host-tuple).2.17
}

check() {
    cd rustowl-${pkgver}
    export RUSTC_BOOTSTRAP=1
    export RUSTUP_TOOLCHAIN=1.86.0
    cargo test --frozen --all-features
}

package() {
    cd rustowl-${pkgver}
    find target -type d | grep -E 'rustowl-build-time-out$' | xargs -I % cp -r % ./
    mkdir sysroot
    ACTIVE_TOOLCHAIN="$(rustup show active-toolchain | awk '{ print $1 }')"
    cp -r "$(rustc --print=sysroot)" sysroot/$ACTIVE_TOOLCHAIN
    find sysroot -type f | grep -v -E '\.(rlib|so|dylib|dll)$' | xargs rm -rf
    find sysroot -depth -type d -empty -exec rm -rf {} \;
    install -d -m 755 "$pkgdir/opt/rustowl"
    cp -a sysroot/ "$pkgdir/opt/rustowl/"
    install -Dm0755 -t "$pkgdir/usr/bin/" "target/$(rustc --print=host-tuple)/release/rustowl"
    install -Dm0755 -t "$pkgdir/usr/bin/" "target/$(rustc --print=host-tuple)/release/rustowlc"
    install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/rustowl/LICENSE"
    install -Dm644 rustowl-build-time-out/man/rustowl.1 "$pkgdir/usr/share/man/man1/rustowl.1"
    install -Dm644 "rustowl-build-time-out/completions/rustowl.bash" "${pkgdir}/usr/share/bash-completion/completions/rustowl"
    install -Dm644 "rustowl-build-time-out/completions/_rustowl" "${pkgdir}/usr/share/zsh/site-functions/_rustowl"
    install -Dm644 "rustowl-build-time-out/completions/rustowl.fish" "${pkgdir}/usr/share/fish/vendor_completions.d/rustowl.fish"
    install -Dm644 "rustowl-build-time-out/completions/rustowl.elv" "${pkgdir}/usr/share/elvish/completions/rustowl.elv"
    install -Dm644 "rustowl-build-time-out/completions/_rustowl.ps1" "${pkgdir}/usr/share/powershell/Modules/Rustowl/_rustowl.ps1"
}

