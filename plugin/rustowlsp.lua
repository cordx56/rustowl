local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

local highlights = {
  lifetime = '#00cc00',
  imm_borrow = '#0000cc',
  mut_borrow = '#cc00cc',
  move = '#cccc00',
  call = '#cccc00',
  outlive = '#cc0000',
}

for hl_name, color in pairs(highlights) do
  local options = { undercurl = true, default = true, sp = color }
  vim.api.nvim_set_hl(0, hl_name, options)
end

if not configs.rustowl then
  configs.rustowl = {
    default_config = {
      cmd = { 'cargo', 'owlsp' },
      root_dir = lspconfig.util.root_pattern('Cargo.toml', '.git'),
      filetypes = { 'rust' },
      on_attach = function(_, _) end,
    },
  }
end
