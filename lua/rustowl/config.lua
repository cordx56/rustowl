---NOTE: `require`ing this module initializes the config

---@class rustowl.Config
---
---Whether to auto-attach the LSP client when opening a Rust file.
---Default: `true`
---@field auto_attach? boolean
---
---Enable RustOwl immediately on LspAttach
---@field auto_enable? boolean
---
---Time in milliseconds to hover with the cursor before triggering RustOwl
---@field idle_time? number
---
---The highlight style to use for underlines ('undercurl' or 'underline')
---Default: `'undercurl'`
---@field highlight_style? string
---
---Custom colors for different highlight types
---@field colors? rustowl.Colors
---
---The LSP client config (This can also be set using |vim.lsp.config()|).
---@field client? rustowl.ClientConfig

---@class rustowl.Colors
---
---Color for lifetime highlights (default: '#00cc00')
---@field lifetime? string
---
---Color for immutable borrow highlights (default: '#0000cc')
---@field imm_borrow? string
---
---Color for mutable borrow highlights (default: '#cc00cc')
---@field mut_borrow? string
---
---Color for move highlights (default: '#cccc00')
---@field move? string
---
---Color for function call highlights (default: '#cccc00')
---@field call? string
---
---Color for outlive error highlights (default: '#cc0000')
---@field outlive? string

---NOTE: This allows lua-language-server to provide users
---completions and hover when setting vim.g.rustowl directly.

---@type nil | rustowl.Config | fun():rustowl.Config
vim.g.rustowl = vim.g.rustowl

---@class rustowl.ClientConfig: vim.lsp.ClientConfig
---
---A function for determining the root directory
---@field root_dir? fun():string()?

---Internal config (defaults), merged with the user config.
---@class rustowl.internal.Config
local default_config = {
  ---@type boolean
  auto_attach = true,

  ---@type boolean
  auto_enable = false,

  ---@type number
  idle_time = 500,

  ---@type string
  highlight_style = 'undercurl',

  ---@type rustowl.Colors
  colors = {
    lifetime = '#00cc00',
    imm_borrow = '#0000cc',
    mut_borrow = '#cc00cc',
    move = '#cccc00',
    call = '#cccc00',
    outlive = '#cc0000',
  },

  ---@class rustowl.internal.ClientConfig: vim.lsp.ClientConfig

  ---
  client = {

    ---@type string[]
    cmd = { 'rustowl' },

    ---@type fun():string?
    root_dir = function()
      return vim.fs.root(0, { 'Cargo.toml', '.git' })
    end,
  },
}

local user_config = type(vim.g.rustowl) == 'function' and vim.g.rustowl() or vim.g.rustowl or {}

---@cast user_config rustowl.Config

---@type rustowl.Config
local lsp_config = type(vim.lsp.config) == 'table' and vim.lsp.config.rustowl or {}

---@type rustowl.internal.Config
local config = vim.tbl_deep_extend('force', default_config, user_config, lsp_config)

vim.validate {
  auto_attach = { config.auto_attach, 'boolean' },
  auto_enable = { config.auto_enable, 'boolean' },
  idle_time = { config.idle_time, 'number' },
  client = { config.client, { 'table' } },
  highlight_style = { config.highlight_style, 'string' },
  colors = { config.colors, 'table' },
}

-- validation for highlight_style to ensure undercurl or underline
if config.highlight_style ~= 'undercurl' and config.highlight_style ~= 'underline' then
  vim.notify(
    "Rustowl: Invalid highlight_style '" .. config.highlight_style .. "'. Using default 'undercurl'.",
    vim.log.levels.WARN
  )
  config.highlight_style = 'undercurl'
end
config.client.name = 'rustowl'

return config
