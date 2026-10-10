{
  description = "RustOwl development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems =
        f: nixpkgs.lib.genAttrs supportedSystems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          name = "rustowl-dev";

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.cmake
            pkgs.gnumake
            pkgs.perl
            pkgs.go
          ];

          buildInputs = [
            pkgs.zlib

            pkgs.clang
            pkgs.gcc

            pkgs.autoconf
            pkgs.automake
            pkgs.libtool
          ];

          shellHook = ''
            export PKG_CONFIG_PATH="${pkgs.zlib.dev}/lib/pkgconfig:${pkgs.openssl.dev}/lib/pkgconfig''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
            export LD_LIBRARY_PATH="${pkgs.zlib.outPath}/lib:${pkgs.openssl.outPath}/lib''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
            for _owl_lib in "$HOME"/.rustowl/sysroot/*/lib/rustlib/*/lib; do
              [ -d "$_owl_lib" ] && export LD_LIBRARY_PATH="$_owl_lib:$LD_LIBRARY_PATH"
            done
            unset _owl_lib

            # jemalloc's autotools configure compiles `strerror_r` probes with
            # -Werror. The NixOS gcc spec injects _FORTIFY_SOURCE without saying
            # so on the command line, and glibc then warns that it needs -O --
            # but jemalloc appends -O0 after its own -O3, so that warning is
            # fatal and configure cannot determine strerror_r's return type.
            # -U_FORTIFY_SOURCE does not work: the spec re-asserts the macro.
            export CFLAGS="-Wno-error=cpp''${CFLAGS:+ $CFLAGS}"
            export HOST_CFLAGS="-Wno-error=cpp''${HOST_CFLAGS:+ $HOST_CFLAGS}"
          '';
        };
      });
    };
}
