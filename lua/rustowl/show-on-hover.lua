local M = {}

local state = {
  augroup = nil,
}

function M.is_enabled()
  return state.augroup ~= nil
end

function M.enable_on_lsp_attach()
  local augroup = vim.api.nvim_create_augroup('RustOwlLspAttach', {})

  vim.api.nvim_create_autocmd('LspAttach', {
    group = augroup,
    callback = function(event)
      M.enable(event.buf)
    end,
  })
end

--- Enable RustOwl highlighting
---@param bufnr? number
function M.enable(bufnr)
  local lsp = require('rustowl.lsp')
  if #lsp.get_rustowl_clients() == 0 then
    lsp.start()
  end
  local idle_time_ms = assert(require('rustowl.config').idle_time)

  local timer = nil

  local function clear_timer()
    if timer then
      timer:stop()
      timer:close()
      timer = nil
    end
  end

  local function start_timer()
    clear_timer()
    local t, err = vim.uv.new_timer()
    timer = t
    assert(timer, err)

    timer:start(
      idle_time_ms,
      0,
      vim.schedule_wrap(function()
        local line, col = unpack(vim.api.nvim_win_get_cursor(0))
        require('rustowl.highlight').enable(line, col, bufnr)
      end)
    )
  end

  state.augroup = vim.api.nvim_create_augroup('RustOwl', { clear = true })

  vim.api.nvim_create_autocmd({ 'CursorMoved', 'CursorMovedI' }, {
    group = state.augroup,
    buffer = bufnr,
    callback = function()
      require('rustowl.highlight').disable(bufnr)
      start_timer()
    end,
  })

  vim.api.nvim_create_autocmd('BufUnload', {
    group = state.augroup,
    buffer = bufnr,
    callback = clear_timer,
  })

  start_timer()
end

--- Disable RustOwl highlighting
---@param bufnr? number
function M.disable(bufnr)
  require('rustowl.highlight').disable(bufnr)

  if M.is_enabled() then
    vim.api.nvim_del_augroup_by_id(state.augroup)
  end

  state.augroup = nil
end

--- Toggle RustOwl highlighting on or off
---@param bufnr? number
function M.toggle(bufnr)
  local action = M.is_enabled() and M.disable or M.enable
  action(bufnr)
end

return M
