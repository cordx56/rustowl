# RustOwl Configuration (Emacs)

## Table Of Contents

<!--toc:start-->
- [RustOwl Configuration (Emacs)](#rustowl-configuration-emacs)
  - [Table Of Contents](#table-of-contents)
  - [Quick Start](#quick-start)
  - [Basic Setup](#basic-setup)
  - [What the package does](#what-the-package-does)
  - [Defaults and variables](#defaults-and-variables)
  - [How highlighting works](#how-highlighting-works)
  - [Enabling / Disabling](#enabling-disabling)
  - [Examples](#examples)
  - [Troubleshooting](#troubleshooting)
<!--toc:end-->

## Quick Start

1. Install RustOwl (see docs/installation.md) and install the Emacs package via `elpaca` or `use-package`.
2. Open a Rust Cargo workspace in Emacs.
3. Place the cursor on a variable and wait ~2 seconds (default) to see overlays with lifetime and borrow info.

This document describes how to use RustOwl from Emacs. It mirrors the detail level of `docs/neovim-configuration.md` and is derived from `rustowl.el`.

## Basic Setup

Install the `rustowl` executable (see `docs/installation.md`). Install the Emacs package via `elpaca` or `use-package`.

Elpaca example:

```elisp
(elpaca
  (rustowl
    :host github
    :repo "cordx56/rustowl"))
```

Use-package example:

```elisp
(use-package rustowl
  :after lsp-mode)
```

## What the package does

- Registers an LSP client for `rust-mode`, `rust-ts-mode`, and `rustic-mode` using `rustowl` as the executable.
- Sends `rustowl/analyze` on save (see `rustowl-enable-analyze-on-save`).
- Sends `rustowl/cursor` when the cursor has been idle for `rustowl-cursor-timeout` seconds (default 2.0 seconds) and applies underlines via overlays.

## Defaults and variables

- `rustowl-cursor-timeout` (float) — default: 2.0 seconds. Idle time before the cursor request is sent.
- `rustowl-cursor-timer` — internal timer variable managed by the package.

## How highlighting works

- The package sends `rustowl/cursor` with position and document URI. The server responds with decorations (type, range, hover text, overlapped).
- For each non-overlapped decoration, the package maps types to underline colors:
  - `lifetime` → `#00cc00` (green)
  - `imm_borrow` → `#0000cc` (blue)
  - `mut_borrow` → `#cc00cc` (purple)
  - `move` / `call` → `#cccc00` (yellow)
  - `outlive` → `#cc0000` (red)
- Underlines are implemented via overlays with face `(:underline (:color <color> :style wave))`.

## Enabling / Disabling

- `enable-rustowl-cursor` — enable cursor-based highlighting for current buffer (adds post-command-hook).
- `disable-rustowl-cursor` — disable cursor-based highlighting (removes hook and cancels timer).
- Cursor highlighting is automatically enabled for Rust buffers via `rust-mode-hook`, `rust-ts-mode-hook`, and `rustic-mode-hook`.

## Examples

To disable analyze-on-save globally, remove the hooks or call:

```elisp
(remove-hook 'rust-mode-hook #'enable-rustowl-cursor)
```

To customize the timeout in your config:

```elisp
(setq rustowl-cursor-timeout 1.0) ; 1 second
```

## Troubleshooting

- If overlays don't appear, ensure `lsp-mode` is active and `rustowl` server is reachable.
- Use `M-x rustowl-clear-overlays` to clear overlays if they persist.

See `docs/lsp-spec.md` for the LSP request/response shapes.
