# Contribution guide

_This document is under construction_.

Thank you for considering to contribute RustOwl!

In this document we describe how to contribute our project, as follows:

- How to setup development environment
- Checklist before submitting PR

## Set up your environment

Here we describe how to set up your development environment.

### Rust code

In the Rust code, we utilize nightly compiler features, which require some tweaks.
Before starting this section, you might be required to install `rustup` since our project requires nightly compiler.

#### Build and test using the nightly environment

For building, testing, or installing, you can do the same as any common Rust project using the `cargo` command.
Here, `cargo` must be a `rustup`-proxied command, which is usually installed with `rustup`.

#### Build with stable Rust compiler

To distribute release binary, we use stable Rust compiler to ship RustOwl with stable Rust compiler for users.

The executable binary named `rustowlc`, which is one of the components of RustOwl, behaves like a Rust compiler.
So we would like to compile `rustowlc`, which uses nightly features, with the stable Rust compiler.

Note: Using this method is strongly discouraged officially. See [Unstable Book](doc.rust-lang.org/nightly/unstable-book/compiler-flags/rustc-bootstrap.html).

To compile `rustowlc` with stable compiler, you should set environment variable as `RUSTC_BOOTSTRAP=1`.

For example building with stable 1.87.0 Rust compiler:

```bash
RUSTC_BOOTSTRAP=1 rustup run 1.87.0 cargo build --release
```

Note that by using normal `cargo` command RustOwl will be built with nightly compiler since there is a `rust-toolchain.toml` which specifies nightly compiler for development environment.

### VS Code extension

For VS Code extension, we use `yarn` to setup environment.
To get started, you have to install dependencies by running following command inside `vscode` directory:

```bash
yarn install
```

## Before submitting PR

Before submitting PR, you have to check below:

### Rust code

- Correctly formatted by `cargo fmt`
- Linted using Clippy by `cargo clippy`

### VS Code extension

- Correctly formatted by `yarn fmt`
