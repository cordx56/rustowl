local M = {}

---@param filter? vim.lsp.get_clients.Filter
---@return vim.lsp.Client[]
M.get_rustowl_clients = function(filter)
  filter = vim.tbl_deep_extend('force', filter or {}, {
    name = 'rustowl',
  })
  return vim.lsp.get_clients(filter)
end

---Start / attach the LSP client
---@return integer|nil client_id The LSP client ID
M.start = function()
  local config = require('rustowl.config')
  return vim.lsp.start(config.client)
end

---Compatibility for a breaking change in Nvim 0.11
---@param client vim.lsp.Client
---@return boolean
local function client_is_stopped(client)
  local info = debug.getinfo(client.is_stopped, 'u')
  if info.nparams > 0 then
    ---@diagnostic disable-next-line: param-type-mismatch
    return client:is_stopped()
  else
    ---@diagnostic disable-next-line: missing-parameter
    return client.is_stopped()
  end
end

M.stop = function()
  local clients = M.get_rustowl_clients()
  vim.lsp.stop_client(clients)
  local t, err, _ = vim.uv.new_timer()
  local timer = assert(t, err)
  local max_attempts = 50
  local attempts_to_live = max_attempts
  local stopped_client_count = 0
  timer:start(200, 100, function()
    for _, client in ipairs(clients) do
      if client_is_stopped(client) then
        stopped_client_count = stopped_client_count + 1
      end
    end
    if stopped_client_count >= #clients then
      timer:stop()
      attempts_to_live = 0
    elseif attempts_to_live <= 0 then
      vim.schedule(function()
        vim.notify(
          ('rustowl: Could not stop all LSP clients after %d attempts.'):format(max_attempts),
          vim.log.levels.ERROR
        )
      end)
      timer:stop()
      attempts_to_live = 0
    end
    attempts_to_live = attempts_to_live - 1
  end)
end

M.restart = function()
  M.stop()
  M.start()
end

return M
