---NOTE: `require`ing this module initializes the config

---@class rustowl.Config: vim.lsp.ClientConfig
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

---NOTE: This allows lua-language-server to provide users
---completions and hover when setting vim.g.rustowl directly.

---@type nil | rustowl.Config | fun():rustowl.Config
vim.g.rustowl = vim.g.rustowl

---Internal config (defaults), merged with the user config.
---@class rustowl.internal.Config
local default_config = {
	---@type boolean
	auto_attach = true,

	---@type boolean
	auto_enable = false,

	---@type number
	idle_time = 500,

	---@type vim.lsp.ClientConfig
	client = {

		---@type string[]
		cmd = { "cargo", "owlsp" },

		---@type fun():string?
		root_dir = function()
			return vim.fs.dirname(vim.fs.find({ "Cargo.toml", ".git" }, { upward = true })[1])
		end,
	},
}

local user_config = type(vim.g.rustowl) == "function" and vim.g.rustowl() or vim.g.rustowl or {}

---@cast user_config rustowl.Config

---@type rustowl.Config
local lsp_config = type(vim.lsp.config) == "table" and vim.lsp.config.rustowl or {}

---@type rustowl.internal.Config
local config = vim.tbl_deep_extend("force", default_config, user_config, lsp_config)

vim.validate({
	auto_attach = { config.auto_attach, "boolean" },
	auto_enable = { config.auto_enable, "boolean" },
	idle_time = { config.idle_time, "number" },
	client = { config.client, { "table" } },
})

config.client.name = "rustowl"

return config
