# RustOwl Configuration (Neovim)

## Quick Start

1. Install RustOwl (see docs/installation.md) and the Neovim plugin via your plugin manager.
2. Add the minimal setup to your lazy.nvim config and open a Rust Cargo workspace.
3. Hover the cursor over a variable and wait ~500ms to see visualizations.

This document describes all available configuration options for the RustOwl Neovim plugin.

## Table of Contents

<!--toc:start-->

- [RustOwl Configuration (Neovim)](#rustowl-configuration-neovim)
  - [Quick Start](#quick-start)
  - [Table of Contents](#table-of-contents)
  - [Basic Setup](#basic-setup)
  - [Configuration Options](#configuration-options)
    - [`auto_attach` (boolean)](#autoattach-boolean)
    - [`auto_enable` (boolean)](#autoenable-boolean)
    - [`idle_time` (number)](#idletime-number)
    - [`highlight_style` (string)](#highlightstyle-string)
    - [`colors` (table)](#colors-table)
      - [Available Color Options:](#available-color-options)
    - [`client` (table)](#client-table)
  - [Customizing Highlight Colors](#customizing-highlight-colors)
    - [Default Colors](#default-colors)
    - [Custom Colors](#custom-colors)
      - [Color Format](#color-format)
    - [Partial Color Customization](#partial-color-customization)
  - [Examples](#examples)
    - [Example 1: Minimal Configuration](#example-1-minimal-configuration)
    - [Example 2: Auto-enable with Custom Colors](#example-2-auto-enable-with-custom-colors)
    - [Example 3: Custom Key Binding and Styling](#example-3-custom-key-binding-and-styling)
    - [Example 4: Dark Theme Optimized Colors](#example-4-dark-theme-optimized-colors)
    - [Example 5: For init.vim Users](#example-5-for-initvim-users)
  - [Usage Commands](#usage-commands)
  <!--toc:end-->

## Basic Setup

The minimal configuration for RustOwl using [lazy.nvim](https://github.com/folke/lazy.nvim):

```lua
{
  'cordx56/rustowl',
  version = '*', -- Latest stable version
  build = 'cargo binstall rustowl',
  lazy = false, -- This plugin is already lazy
  opts = {},
}
```

## Configuration Options

All configuration options are optional and have sensible defaults:

### `auto_attach` (boolean)

- **Default**: `true`
- **Description**: Automatically attach the RustOwl LSP client when opening a Rust file.

### `auto_enable` (boolean)

- **Default**: `false`
- **Description**: Enable RustOwl highlighting immediately when the LSP client attaches. When `false`, you need to manually enable highlighting using `:Rustowl enable` or `require('rustowl').enable()`.

### `idle_time` (number)

- **Default**: `500`
- **Description**: Time in milliseconds to hover with the cursor before triggering RustOwl analysis and highlighting.

### `highlight_style` (string)

- **Default**: `'undercurl'`
- **Options**: `'undercurl'` or `'underline'`
- **Description**: The style of underline to use for highlighting.

### `colors` (table)

- **Description**: Custom colors for different highlight types. Each color should be a hex color string (e.g., `'#ff0000'`).

#### Available Color Options:

- `lifetime`: Color for variable lifetime highlights (default: `'#00cc00'` - green)
- `imm_borrow`: Color for immutable borrow highlights (default: `'#0000cc'` - blue)
- `mut_borrow`: Color for mutable borrow highlights (default: `'#cc00cc'` - purple)
- `move`: Color for value move highlights (default: `'#cccc00'` - yellow)
- `call`: Color for function call highlights (default: `'#cccc00'` - yellow)
- `outlive`: Color for lifetime error highlights (default: `'#cc0000'` - red)

### `client` (table)

- **Description**: LSP client configuration that gets passed to `vim.lsp.start`. This follows the same structure as Neovim's LSP client configuration.

## Customizing Highlight Colors

### Default Colors

The default color scheme uses the following colors that correspond to the visual legend:

- 🟩 **Green** (`#00cc00`): Variable's actual lifetime
- 🟦 **Blue** (`#0000cc`): Immutable borrowing
- 🟪 **Purple** (`#cc00cc`): Mutable borrowing
- 🟧 **Yellow** (`#cccc00`): Value moved / function call
- 🟥 **Red** (`#cc0000`): Lifetime errors

### Custom Colors

To customize colors, specify them in the `colors` table:

```lua
opts = {
  colors = {
    lifetime = '#32cd32',   -- Lime green
    imm_borrow = '#4169e1', -- Royal blue
    mut_borrow = '#ff69b4', -- Hot pink
    move = '#ffa500',       -- Orange
    call = '#ffd700',       -- Gold
    outlive = '#dc143c',    -- Crimson
  },
}
```

#### Color Format

Colors must be specified as hex color strings:

- ✅ Valid: `'#ff0000'`, `'#00ff00'`, `'#0000ff'`
- ❌ Invalid: `'red'`, `'rgb(255,0,0)'`, `'#f00'`

### Partial Color Customization

You can customize only specific colors while keeping the defaults for others:

```lua
opts = {
  colors = {
    lifetime = '#90ee90',   -- Light green for better visibility
    outlive = '#ff4500',    -- Orange red for errors
    -- Other colors will use defaults
  },
}
```

## Examples

### Example 1: Minimal Configuration

```lua
{
  'cordx56/rustowl',
  version = '*',
  build = 'cargo binstall rustowl',
  lazy = false,
  opts = {},
}
```

### Example 2: Auto-enable with Custom Colors

```lua
{
  'cordx56/rustowl',
  version = '*',
  build = 'cargo binstall rustowl',
  lazy = false,
  opts = {
    auto_enable = true,
    colors = {
      lifetime = '#90ee90',   -- Light green
      imm_borrow = '#87ceeb', -- Sky blue
      mut_borrow = '#dda0dd', -- Plum
      move = '#f0e68c',       -- Khaki
      call = '#ffd700',       -- Gold
      outlive = '#ff6347',    -- Tomato
    },
  },
}
```

### Example 3: Custom Key Binding and Styling

```lua
{
  'cordx56/rustowl',
  version = '*',
  build = 'cargo binstall rustowl',
  lazy = false,
  opts = {
    auto_enable = false,
    idle_time = 300,
    highlight_style = 'underline',
    colors = {
      outlive = '#ff0000', -- Bright red for errors
    },
    client = {
      on_attach = function(_, buffer)
        vim.keymap.set('n', '<leader>ro', function()
          require('rustowl').toggle(buffer)
        end, { buffer = buffer, desc = 'Toggle RustOwl' })

        vim.keymap.set('n', '<leader>re', function()
          require('rustowl').enable(buffer)
        end, { buffer = buffer, desc = 'Enable RustOwl' })

        vim.keymap.set('n', '<leader>rd', function()
          require('rustowl').disable(buffer)
        end, { buffer = buffer, desc = 'Disable RustOwl' })
      end
    },
  },
}
```

### Example 4: Dark Theme Optimized Colors

```lua
{
  'cordx56/rustowl',
  version = '*',
  build = 'cargo binstall rustowl',
  lazy = false,
  opts = {
    colors = {
      lifetime = '#50fa7b',   -- Dracula green
      imm_borrow = '#8be9fd', -- Dracula cyan
      mut_borrow = '#ff79c6', -- Dracula pink
      move = '#f1fa8c',       -- Dracula yellow
      call = '#ffb86c',       -- Dracula orange
      outlive = '#ff5555',    -- Dracula red
    },
  },
}
```

### Example 5: For init.vim Users

If you're using `init.vim` instead of `init.lua`, you can configure RustOwl using Vim script:

```vim
lua << EOF
require('lazy').setup({
  {
    'cordx56/rustowl',
    version = '*',
    build = 'cargo binstall rustowl',
    lazy = false,
    opts = {
      colors = {
        lifetime = '#00ff00',
        imm_borrow = '#0080ff',
        mut_borrow = '#ff00ff',
        move = '#ffff00',
        call = '#ffa500',
        outlive = '#ff0000',
      },
    },
  },
})
EOF
```

## Usage Commands

When opening a Rust file, the following commands become available:

- `:Rustowl start_client` - Start the RustOwl LSP client
- `:Rustowl stop_client` - Stop the RustOwl LSP client
- `:Rustowl restart_client` - Restart the RustOwl LSP client
- `:Rustowl enable` - Enable RustOwl highlighting
- `:Rustowl disable` - Disable RustOwl highlighting
- `:Rustowl toggle` - Toggle RustOwl highlighting

You can also use the Lua API:

```lua
require('rustowl').enable()   -- Enable highlighting
require('rustowl').disable()  -- Disable highlighting
require('rustowl').toggle()   -- Toggle highlighting
require('rustowl').is_enabled() -- Check if enabled
```
