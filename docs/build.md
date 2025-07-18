# Build guide

Here we describe how to build each of our repository.

## RustOwl

You can choose one of two ways to build RustOwl.

You can add runtime directory paths to the search paths by specifying the `RUSTOWL_RUNTIME_DIRS` or `RUSTOWL_SYSROOTS` environment variables.
The default runtime directory is `$HOME/.rustowl`.

On a freshly installed Ubuntu system, you need to run `apt install build-essential` to ensure all required build tools are available for linking.

### Build RustOwl using stable toolchain

There are scripts to build the stable version of RustOwl.
`scripts/build/toolchain` sets up the RustOwl toolchain and executes command using that toolchain.

```bash
./scripts/build/toolchain cargo install --path . --locked
```

### Build RustOwl using custom toolchain

You can build RustOwl using a custom toolchain. This method is recommended for contributors to RustOwl.

The prerequisites are as follows:

- `rustup` installed
  - You can install `rustup` from [this link](https://rustup.rs/).
  - You need to set up the `PATH` environment variable. Follow the instructions provided by the `rustup` installer to do this.
- `gcc` or `clang` installed.
  - On Windows, you can install the Visual Studio toolchain instead.

Building RustOwl requires a nightly build of `rustc`. It will be installed automatically by `rustup` if needed.

Other dependencies are specified in the configuration files and will be installed automatically.

```bash
cargo install --path . --locked
```

## VSCode extension

### Prerequisite

- VS Code installed
  - You can install VS Code from [this link](https://code.visualstudio.com/).
- Node.js installed
- `yarn` installed
  - After installing Node.js, You can install `yarn` by running `npm install -g yarn`.

VS Code extension has been tested on macOS Sequoia 15.3.2 on arm64 architecture with Visual Studio Code 1.99.3, Node.js v20.16.0, and `yarn` 1.22.22.
Other dependencies are locked in the configuration files and will be installed automatically.

### Build & Run

First, install the dependencies:

```bash
cd vscode
yarn install --frozen-lockfile
```

Then, open the `vscode` directory in VS Code.

A notification to install the recommended VS Code extension will appear in the bottom right corner of VS Code.
Click the install button, wait for the installation to finish, and then restart VS Code.

Open the `vscode` directory again, and press the `F5` key in the VS Code window.
A new VS Code window with the extension enabled will appear.

Open the cargo workspace directory in the new VS Code window.

When you save Rust files, decorations indicating the movement of ownership and lifetimes will appear in the editor.
