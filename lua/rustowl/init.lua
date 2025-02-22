local M = {}

---@class RustOwlOptions
---@field auto_enable? boolean Enable RustOwl immediately
---@field idle_time? number Time in milliseconds to hover with the cursor before triggering RustOwl
---@field client? vim.lsp.ClientConfig | {} LSP client configuration that gets passed to `require('lspconfig').rustowl.setup()`
local options = {
  auto_enable = true,
  idle_time = 500,
  client = {},
}

---@return RustOwlOptions
function M.get_options()
  return options
end

M.enable = require('rustowl.show-on-hover').enable

M.disable = require('rustowl.show-on-hover').disable

M.toggle = require('rustowl.show-on-hover').toggle

---@param opts? RustOwlOptions
function M.setup(opts)
  ---@type RustOwlOptions
  options = vim.tbl_deep_extend('keep', opts or {}, options)
  require('lspconfig').rustowl.setup(options.client)

  if options.auto_enable then
    require('rustowl.show-on-hover').enable_on_lsp_attach()
  end
end

return M
