# Installation

## Table Of Contents

<!--toc:start-->
- [Installation](#installation)
  - [Table Of Contents](#table-of-contents)
  - [Quick Start](#quick-start)
  - [Prerequisites](#prerequisites)
  - [cargo-binstall (recommended)](#cargo-binstall-recommended)
  - [Windows (winget)](#windows-winget)
  - [Arch Linux (AUR)](#arch-linux-aur)
  - [Nix flake](#nix-flake)
  - [GitHub Releases](#github-releases)
  - [Docker](#docker)
  - [Build manually](#build-manually)
  <!--toc:end-->

## Quick Start

- Install via cargo-binstall (recommended):

```bash
cargo binstall rustowl
```

- Or install via your platform package manager (winget, AUR) or download from GitHub Releases. See below for full options.

This document collects supported installation methods and examples.

## Prerequisites

- Rust toolchain (cargo). Install via rustup: https://rustup.rs/

## cargo-binstall (recommended)

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
