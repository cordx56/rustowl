# Installation

## Table Of Contents

<!--toc:start-->
- [Installation](#installation)
  - [Table Of Contents](#table-of-contents)
  - [Quick Start](#quick-start)
  - [Runtime dependencies](#runtime-dependencies)
    - [Windows](#windows)
    - [Linux](#linux)
    - [macOS](#macos)
  - [cargo-binstall (recommended)](#cargo-binstall)
  - [Windows (winget)](#windows-winget)
  - [Arch Linux (AUR)](#arch-linux-aur)
  - [Nix flake](#nix-flake)
  - [GitHub Releases](#github-releases)
  - [Docker](#docker)
  - [Build manually](#build-manually)
  <!--toc:end-->

## Quick Start

- Install using our script (recommended):

```
curl -L https://raw.githubusercontent.com/cordx56/rustowl/refs/heads/main/scripts/installer | sh
```

## Runtime dependencies

RustOwl ships two executables:

- `rustowl` is the LSP server that editor extensions talk to.
- `rustowlc` is the Rust compiler that performs the analysis.

Those prebuilt compiler binaries are dynamically linked against a few system
libraries that are not bundled, so they must be present on the machine that runs
RustOwl. This is a runtime requirement only — building RustOwl from source does
not need them.

### Windows

The prebuilt compiler binaries are built with the MSVC toolchain, so they require
the Microsoft Visual C++ 2015-2022 Redistributable (x64). It is probably already installed
on your machine. If not, you can install it with winget:

```sh
winget install Microsoft.VCRedist.2015+.x64
```

You can also download the installer directly from
<https://aka.ms/vs/17/release/vc_redist.x64.exe>.

Without it, launching RustOwl fails immediately with:

```
The code execution cannot proceed because VCRedist140_1.dll was not found.
```

### Linux

The prebuilt `libLLVM` shipped by the Rust project has a system dependency on the
zlib shared library (`libz.so.1`). Install it with your package manager:

| Distribution | Command |
| --- | --- |
| Debian, Ubuntu | `sudo apt-get install -y zlib1g` |
| Fedora, RHEL | `sudo dnf install -y zlib` |
| Arch Linux | `sudo pacman -S zlib` (already in `base`) |
| Alpine | `sudo apk add zlib` |

Without it, RustOwl fails immediately with:

```
rustowlc: error while loading shared libraries: libz.so.1: cannot open shared object file: No such file or directory
```

### macOS

Nothing is needed.

## cargo-binstall

Install the prebuilt binary using cargo-binstall:

```bash
cargo binstall rustowl
```

This automatically downloads and unpacks a Rust toolchain if required.

## Windows (winget)

Install with:

```sh
winget install rustowl
```

## Arch Linux (AUR)

We provide AUR packages that either install prebuilt binaries or build from source.
Prebuilt binaries (recommended):

```sh
yay -S rustowl-bin
```

Build from AUR (cargo build):

```sh
yay -S rustowl
```

Git (build from latest source):

```sh
yay -S rustowl-git
```

Replace `yay` with your AUR helper of choice.

## Nix flake

There is a [third-party Nix flake repository](https://github.com/nix-community/rustowl-flake) in the Nix community.

## GitHub Releases

Download the `rustowl` executable from the release page:

https://github.com/cordx56/rustowl/releases/latest

Place the executable into a directory on your PATH.

## Docker

Run the prebuilt image from GitHub Container Registry:

```sh
docker pull ghcr.io/cordx56/rustowl:latest
```

Run it against a project directory:

```sh
docker run --rm -v /path/to/project:/app ghcr.io/cordx56/rustowl:latest
```

Use an alias to act like a local CLI:

```sh
alias rustowl='docker run --rm -v $(pwd):/app ghcr.io/cordx56/rustowl:latest'
```

## Build manually

See `docs/build.md` for detailed build instructions and how to build editor extensions.
