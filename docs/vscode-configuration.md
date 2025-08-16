# RustOwl Configuration (VS Code)

## Table Of Contents

<!--toc:start-->

- [RustOwl Configuration (VS Code)](#rustowl-configuration-vs-code)
  - [Table Of Contents](#table-of-contents)
  - [Quick Start](#quick-start)
  - [Configuration](#configuration)
  - [Settings (configuration keys)](#settings-configuration-keys)
  - [Commands](#commands)
  - [Behavior](#behavior)
  - [Examples](#examples)
  - [Troubleshooting](#troubleshooting)
  <!--toc:end-->

This document describes configuration options for the RustOwl VS Code extension.

## Quick Start

Install the extension from the Marketplace or Open VSX. The extension will download and run the rustowl binary automatically when activated.


1. Install the extension in VS Code.
2. Open a Rust Cargo workspace.
3. Save a Rust file and hover over a variable after a short idle time to see decorations.

## Configuration

The VS Code extension exposes configurable settings and commands. Here are the concrete settings and behavior derived from code.

## Settings (configuration keys)

- `rustowl.underlineThickness` (string) — The stroke thickness of the underline line. Allowed values: "1", "2", "3", "4".
- `rustowl.lifetimeColor` (string) — The color of the lifetime.
- `rustowl.moveCallColor` (string) — The color of the move/call.
- `rustowl.immutableBorrowColor` (string) — The color of the immutable borrow.
- `rustowl.mutableBorrowColor` (string) — The color of the mutable borrow.
- `rustowl.outliveColor` (string) — The color of the outlive.
- `rustowl.displayDelay` (number) — Delay in displaying underlines (ms).
- `rustowl.highlightBackground` (boolean) — Highlight text background instead of underline.
- `rustowl.defaultEnabled` (boolean) — Enabled by default.

## Commands

- `rustowl.hover` — manually request hover/decorations for current selection.
- `rustowl.toggle` — toggle decorations on/off (status bar changes accordingly).

## Behavior

- The extension bootstraps the `rustowl` binary using `bootstrap.ts` and starts an LSP client pointing at the executable.
- On save of Rust files the extension sends `rustowl/analyze` to the server (if enabled).
- On cursor selection changes, after `displayDelay` ms the extension sends `rustowl/cursor` and applies decorations returned by server.
- Decorations ignore overlapped ranges and map LSP response types to decorations (lifetime, imm_borrow, mut_borrow, move/call, outlive/shared_mut).

## Examples

Add settings to `.vscode/settings.json`:

```json
{
  "rustowl.displayDelay": 2000,
  "rustowl.defaultEnabled": true,
  "rustowl.lifetimeColor": "#00cc00",
  "rustowl.immutableBorrowColor": "#0000cc",
  "rustowl.mutableBorrowColor": "#cc00cc",
  "rustowl.moveCallColor": "#cccc00",
  "rustowl.outliveColor": "#cc0000",
  "rustowl.highlightBackground": false,
  "rustowl.underlineThickness": 2
}
```

## Troubleshooting

- Check Output -> RustOwl for logs. If the server failed to start, the extension shows an error message.
- Ensure the `rustowl` executable is in PATH or let the extension download it (bootstrap).
