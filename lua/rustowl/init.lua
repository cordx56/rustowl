local M = {}

--- Enable RustOwl highlighting
---@param bufnr? number
M.enable = function(bufnr)
	require("rustowl.show-on-hover").enable(bufnr)
end

--- Disable RustOwl highlighting
---@param bufnr? number
M.disable = function(bufnr)
	require("rustowl.show-on-hover").disable(bufnr)
end

--- Toggle RustOwl highlighting on or off
---@param bufnr? number
M.toggle = function(bufnr)
	require("rustowl.show-on-hover").toggle(bufnr)
end

---@return true if rustowl highlighting is enabled
M.is_enabled = function()
	return require("rustowl.show-on-hover").is_enabled()
end

---@param opts? rustowl.Config
function M.setup(opts)
	vim.g.rustowl = opts
end

return M
