local M = {}

local hl_ns = vim.api.nvim_create_namespace('rustowl')

---@param line number
---@param col number
---@param bufnr? number
function M.enable(line, col, bufnr)
  local lsp = require('rustowl.lsp')
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local clients = lsp.get_rustowl_clients { bufnr = bufnr }
  local params = {
    position = { line = line - 1, character = col },
    document = vim.lsp.util.make_text_document_params(),
  }

  for _, client in ipairs(clients) do
    client:request('rustowl/cursor', params, function(_, result, _)
      if result ~= nil then
        for _, deco in ipairs(result['decorations']) do
          local start = { deco['range']['start']['line'], deco['range']['start']['character'] }
          local finish = { deco['range']['end']['line'], deco['range']['end']['character'] }
          local opts = { regtype = 'v', inclusive = true }
          vim.highlight.range(bufnr, hl_ns, deco['type'], start, finish, opts)
        end
      end
    end, bufnr)
  end
end

---@param bufnr? number
function M.disable(bufnr)
  vim.api.nvim_buf_clear_namespace(bufnr or 0, hl_ns, 0, -1)
end

return M
