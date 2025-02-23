local config = require('rustowl.config')

if not vim.g.loaded_rustowl then
  -- Plugin initialization (run only once)
  vim.g.loaded_rustowl = true

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

  if config.auto_enable then
    require('rustowl.show-on-hover').enable_on_lsp_attach()
  end

  ---@enum rustowl.ClientCmd
  local RustowlCmd = {
    start_client = 'start_client',
    stop_client = 'stop_client',
    restart_client = 'restart_client',
    enable = 'enable',
    disable = 'disable',
    toggle = 'toggle',
  }

  local lsp = require('rustowl.lsp')
  local rustowl = require('rustowl')

  local function rustowl_user_cmd(opts)
    if vim.bo[0].filetype ~= 'rust' then
      vim.notify(
        'Rustowl: Current buffer is not a rust file.',
        vim.log.levels.ERROR
      )
      return
    end
    local fargs = opts.fargs
    local cmd = fargs[1]
    ---@cast cmd rustowl.ClientCmd
    if cmd == RustowlCmd.start_client then
      lsp.start()
    elseif cmd == RustowlCmd.stop_client then
      lsp.stop()
    elseif cmd == RustowlCmd.restart_client then
      lsp.restart()
    elseif cmd == RustowlCmd.enable then
      rustowl.enable()
    elseif cmd == RustowlCmd.disable then
      rustowl.disable()
    elseif cmd == RustowlCmd.toggle then
      rustowl.toggle()
    end
  end

  vim.api.nvim_create_user_command('Rustowl', rustowl_user_cmd, {
    nargs = '+',
    desc = 'Starts, stops the rustowl LSP client',
    complete = function(arg_lead, cmdline, _)
      local clients = lsp.get_rustowl_clients()
      ---@type rustowl.ClientCmd[]
      local commands = {}
      if #clients == 0 then
        table.insert(commands, RustowlCmd.start_client)
      else
        table.insert(commands, RustowlCmd.toggle)
        if rustowl.is_enabled() then
          table.insert(commands, RustowlCmd.disable)
        else
          table.insert(commands, RustowlCmd.enable)
        end
        table.insert(commands, RustowlCmd.stop_client)
        table.insert(commands, RustowlCmd.restart_client)
      end
      if cmdline:match('^Rustowl%s+%w*$') then
        return vim.tbl_filter(function(command)
          return command:find(arg_lead) ~= nil
        end, commands)
      end
    end,
  })
end

if config.auto_attach then
  require('rustowl.lsp').start()
end
