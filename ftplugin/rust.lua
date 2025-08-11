local config = require('rustowl.config')

if not vim.g.loaded_rustowl then
  vim.g.loaded_rustowl = true

  local highlight_style = config.highlight_style or 'undercurl'

  local default_colors = {
    lifetime = '#00cc00',
    imm_borrow = '#0000cc',
    mut_borrow = '#cc00cc',
    move = '#cccc00',
    call = '#cccc00',
    outlive = '#cc0000',
  }

  -- Ensure all color keys are present even if user provides a partial table
  local highlights = vim.tbl_deep_extend('keep', config.colors or {}, default_colors)

  for hl_name, color in pairs(highlights) do
    local options = { default = true, sp = color }
    if highlight_style == 'underline' then
      options.underline = true
    else
      options.undercurl = true
    end
    vim.api.nvim_set_hl(0, hl_name, options)
  end

  vim.api.nvim_create_user_command('Rustowl', function(opts)
    if vim.bo[0].filetype ~= 'rust' then
      vim.notify('Rustowl: Current buffer is not a rust file.', vim.log.levels.ERROR)
      return
    end
    local fargs = opts.fargs
    local cmd = fargs[1]
    if cmd == 'start_client' then
      require('rustowl.lsp').start()
    elseif cmd == 'stop_client' then
      require('rustowl.lsp').stop()
    elseif cmd == 'restart_client' then
      require('rustowl.lsp').restart()
    elseif cmd == 'enable' then
      require('rustowl').enable()
    elseif cmd == 'disable' then
      require('rustowl').disable()
    elseif cmd == 'toggle' then
      require('rustowl').toggle()
    end
  end, {
    nargs = '+',
    desc = 'Starts, stops the rustowl LSP client',
    complete = function(arg_lead, cmdline, _)
      local lsp = require('rustowl.lsp')
      local rustowl = require('rustowl')
      local clients = lsp.get_rustowl_clients()
      local commands = {}
      if #clients == 0 then
        table.insert(commands, 'start_client')
      else
        table.insert(commands, 'toggle')
        if rustowl.is_enabled() then
          table.insert(commands, 'disable')
        else
          table.insert(commands, 'enable')
        end
        table.insert(commands, 'stop_client')
        table.insert(commands, 'restart_client')
      end
      if cmdline:match('^Rustowl%s+%w*$') then
        return vim.tbl_filter(function(command)
          return command:find(arg_lead) ~= nil
        end, commands)
      end
    end,
  })

  -- Send analyze request on save
  local analyze_augroup = vim.api.nvim_create_augroup('RustOwlAnalyzeOnSave', { clear = false })
  vim.api.nvim_create_autocmd('BufWritePost', {
    group = analyze_augroup,
    buffer = 0,
    desc = 'RustOwl: send rustowl/analyze on save',
    callback = function(args)
      -- Only send when rustowl is enabled
      local rustowl = require('rustowl')
      if not rustowl.is_enabled() then
        return
      end
      local lsp = require('rustowl.lsp')
      local clients = lsp.get_rustowl_clients { bufnr = args.buf }
      for _, client in ipairs(clients) do
        client:request('rustowl/analyze', {}, function() end, args.buf)
      end
    end,
  })
end

if config.auto_enable then
  require('rustowl.show-on-hover').enable_on_lsp_attach()
end

if config.auto_attach then
  require('rustowl.lsp').start()
end
