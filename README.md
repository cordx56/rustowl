<div align="center">
    <h1>
      <picture>
        <source media="(prefers-color-scheme: dark)" srcset="docs/assets/rustowl-logo-dark.svg">
        <img alt="RustOwl" src="docs/assets/rustowl-logo.svg" width="400">
      </picture>
    </h1>
    <p>
      Visualize ownership and lifetimes in Rust for debugging and optimization
    </p>
    <p>
      You can try out RustOwl on <a href="https://play.rustowl.rs/">play.rustowl.rs</a>
    </p>
    <p>
      <font size="4">
          <a href="https://crates.io/crates/rustowl">
              <img alt="Crates.io Version" src="https://img.shields.io/crates/v/rustowl?style=for-the-badge">
          </a>
          <a href="https://aur.archlinux.org/packages/rustowl-bin">
              <img alt="AUR Version" src="https://img.shields.io/aur/version/rustowl-bin?style=for-the-badge">
          </a>
          <img alt="WinGet Package Version" src="https://img.shields.io/winget/v/Cordx56.Rustowl?style=for-the-badge">
      </font>
    </p>
    <p>
      <font size="4">
          <a href="https://marketplace.visualstudio.com/items?itemName=cordx56.rustowl-vscode">
              <img alt="Visual Studio Marketplace Version" src="https://img.shields.io/visual-studio-marketplace/v/cordx56.rustowl-vscode?style=for-the-badge&label=VS%20Code">
          </a>
          <a href="https://open-vsx.org/extension/cordx56/rustowl-vscode">
              <img alt="Open VSX Version" src="https://img.shields.io/open-vsx/v/cordx56/rustowl-vscode?style=for-the-badge">
          </a>
          <a href="https://github.com/siketyan/intellij-rustowl">
              <img alt="JetBrains Plugin Version" src="https://img.shields.io/jetbrains/plugin/v/26504-rustowl?style=for-the-badge">
          </a>
      </font>
    </p>
    <p>
      <font size="4">
          <a href="https://discord.gg/XbxN949dpG">
              <img alt="Discord" src="https://img.shields.io/discord/1379759912942436372?style=for-the-badge&logo=discord">
          </a>
      </font>
    </p>
    <p>
        <img src="docs/assets/readme-screenshot-3.png" />
    </p>
</div>

RustOwl visualizes ownership movement and lifetimes of variables.
When you save Rust source code, it is analyzed, and the ownership and lifetimes of variables are visualized when you hover over a variable or function call.

RustOwl visualizes those by using underlines:

- 🟩 green: variable's actual lifetime
- 🟦 blue: immutable borrowing
- 🟪 purple: mutable borrowing
- 🟧 orange: value moved / function call
- 🟥 red: lifetime error
  - Diff of lifetime between actual and expected, or
  - Invalid overlapped lifetime of mutable and shared (immutable) references

Detailed usage is described [here](docs/usage.md).

Currently, we offer VSCode extension, Neovim plugin and Emacs package.
For these editors, move the text cursor over the variable or function call you want to inspect and wait for 2 seconds to visualize the information.
We implemented LSP server with an extended protocol.
So, RustOwl can be used easily from other editor.

## Table Of Contents

<!--toc:start-->

- [Table Of Contents](#table-of-contents)
- [Support](#support)
- [Quick Start](#quick-start)
  - [Prerequisite](#prerequisite)
  - [VS Code](#vs-code)
  - [Vscodium](#vscodium)
- [Other editor support](#other-editor-support)
  - [Neovim](#neovim)
  - [Emacs](#emacs)
  - [RustRover / IntelliJ IDEs](#rustrover--intellij-ides)
  - [Sublime Text](#sublime-text)
- [Installation](#installation)
- [Usage](#usage)
- [Note](#note)
<!--toc:end-->

## Support

If you're looking for support, please consider checking all issues, existing discussions, and [starting a discussion](https://github.com/cordx56/rustowl/discussions/new?category=q-a) first!

Also, you can reach out to us on the Discord server provided above.

## Quick Start

Here we describe how to start using RustOwl.

### Prerequisite

- `cargo` installed
  - You can install `cargo` using `rustup` from [this link](https://rustup.rs/).
- Visual Studio Code (VS Code) installed

We tested this guide on macOS Sequoia 15.3.2 on arm64 architecture with VS Code 1.99.3 and `cargo` 1.89.0.

### VS Code

You can install VS Code extension from [this link](https://marketplace.visualstudio.com/items?itemName=cordx56.rustowl-vscode).
RustOwl will be installed automatically when the extension is activated.

For more detailed configuration options, see the [VS Code Configuration Guide](docs/vscode-configuration.md).

### Vscodium

You can install Vscodium extension from [this link](https://open-vsx.org/extension/cordx56/rustowl-vscode).
RustOwl will be installed automatically when the extension is activated.

After installation, the extension will automatically run RustOwl when you save any Rust program in cargo workspace.
The initial analysis may take some time, but from the second run onward, compile caching is used to reduce the analysis time.

Same as [VS Code](#vs-code), see the [VS Code Configuration Guide](docs/vscode-configuration.md) for more detailed configuration options.

## Other editor support

We support Neovim and Emacs.
You have to [install RustOwl](./docs/installation.md) before using RustOwl with other editors.

You can also create your own LSP client.
If you would like to implement a client, please refer to the [The RustOwl LSP specification](docs/lsp-spec.md).

### Neovim

Minimal setup with [lazy.nvim](https://github.com/folke/lazy.nvim):

```lua
{
  'cordx56/rustowl',
  version = '*', -- Latest stable version
  build = 'cargo install rustowl',
  lazy = false, -- This plugin is already lazy
  opts = {},
}
```

For comprehensive configuration options including custom highlight colors, see the [Neovim Configuration Guide](docs/neovim-configuration.md).

### Emacs

Elpaca example:

```elisp
(elpaca
  (rustowl
    :host github
    :repo "cordx56/rustowl"))
```

Then use-package:

```elisp
(use-package rustowl
  :after lsp-mode)
```

You have to install RustOwl LSP server manually.

For more detailed configuration options, see the [Emacs Configuration Guide](./docs/emacs-configuration.md).

### RustRover / IntelliJ IDEs

There is a [third-party repository](https://github.com/siketyan/intellij-rustowl) that supports IntelliJ IDEs.
You have to install RustOwl LSP server manually.

### Sublime Text

There is a [third-party repository](https://github.com/CREAsTIVE/LSP-rustowl) that supports Sublime Text.

## Installation

Please see [Installation](docs/installation.md) for detailed installation instructions.

## Usage

Please see [Usage](docs/usage.md) for detailed usage instructions.

## Note

In this tool, due to the limitations of VS Code's decoration specifications, characters with descenders, such as g or parentheses, may occasionally not display underlines properly.
Additionally, we observed that the `println!` macro sometimes produces extra output, though this does not affect usability in any significant way.
