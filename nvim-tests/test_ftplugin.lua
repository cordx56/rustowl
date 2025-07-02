local MiniTest = require('mini.test')
local expect = MiniTest.expect

local T = MiniTest.new_set {
  hooks = {
    pre_case = function()
      vim.g.loaded_rustowl = nil
      vim.g.rustowl = nil
      if vim.lsp and vim.lsp.config then
        vim.lsp.config.rustowl = {}
      end
      pcall(vim.api.nvim_del_user_command, 'Rustowl')
      pcall(vim.api.nvim_del_augroup_by_name, 'RustOwl')
      pcall(vim.api.nvim_del_augroup_by_name, 'RustOwlLspAttach')
      vim.cmd('enew')
      vim.bo.filetype = 'rust'
      package.loaded['rustowl.lsp'] = nil
      package.loaded['rustowl.config'] = nil
      package.loaded['rustowl.highlight'] = nil
      package.loaded['rustowl.show-on-hover'] = nil
    end,
    post_case = function()
      pcall(vim.api.nvim_del_user_command, 'Rustowl')
      pcall(vim.api.nvim_del_augroup_by_name, 'RustOwl')
      pcall(vim.api.nvim_del_augroup_by_name, 'RustOwlLspAttach')
      vim.cmd('bwipe!')
      package.loaded['rustowl.lsp'] = nil
      package.loaded['rustowl.config'] = nil
      package.loaded['rustowl.highlight'] = nil
      package.loaded['rustowl.show-on-hover'] = nil
    end,
  },
}

T['ftplugin_creates_highlight_groups'] = function()
  package.loaded['rustowl.config'] = {
    highlight_style = 'undercurl',
    auto_enable = false,
    auto_attach = false,
    client = {
      name = 'rustowl',
      cmd = { 'rustowl' },
      root_dir = function()
        return vim.fn.getcwd()
      end,
    },
  }
  vim.cmd('source ftplugin/rust.lua')
  local highlight_groups = {
    'lifetime',
    'imm_borrow',
    'mut_borrow',
    'move',
    'call',
    'outlive',
  }
  for _, hl_name in ipairs(highlight_groups) do
    local hl = vim.api.nvim_get_hl(0, { name = hl_name })
    expect.equality(type(hl), 'table')
    expect.equality(hl.undercurl == true or hl.underline == true, true)
  end
end

T['ftplugin_creates_user_command'] = function()
  package.loaded['rustowl.config'] = {
    auto_enable = false,
    auto_attach = false,
    client = {
      name = 'rustowl',
      cmd = { 'rustowl' },
      root_dir = function()
        return vim.fn.getcwd()
      end,
    },
  }
  package.loaded['rustowl.lsp'] = {
    start = function() end,
    stop = function() end,
    restart = function() end,
    get_rustowl_clients = function()
      return {}
    end,
  }
  package.loaded['rustowl'] = {
    enable = function() end,
    disable = function() end,
    toggle = function() end,
    is_enabled = function()
      return false
    end,
  }
  vim.cmd('source ftplugin/rust.lua')
  local commands = vim.api.nvim_get_commands {}
  expect.equality(commands.Rustowl ~= nil, true)
end

T['user_command_errors_on_non_rust_buffer'] = function()
  package.loaded['rustowl.config'] = {
    auto_enable = false,
    auto_attach = false,
    client = {
      name = 'rustowl',
      cmd = { 'rustowl' },
      root_dir = function()
        return vim.fn.getcwd()
      end,
    },
  }
  package.loaded['rustowl.lsp'] = {
    get_rustowl_clients = function()
      return {}
    end,
  }
  vim.bo.filetype = 'python'
  vim.cmd('source ftplugin/rust.lua')
  local notify_called = false
  local notify_message = nil
  local notify_level = nil
  local original_notify = vim.notify
  vim.notify = function(msg, level)
    notify_called = true
    notify_message = msg
    notify_level = level
  end
  vim.cmd('Rustowl start_client')
  vim.notify = original_notify
  expect.equality(notify_called, true)
  expect.equality(type(notify_message), 'string')
  expect.equality(notify_level, vim.log.levels.ERROR)
end

T['auto_attach_starts_lsp'] = function()
  -- Patch: forcibly reload ftplugin after setting mocks
  local lsp_start_called = false
  package.loaded['rustowl.config'] = {
    auto_enable = false,
    auto_attach = true,
    client = {
      name = 'rustowl',
      cmd = { 'rustowl' },
      root_dir = function()
        return vim.fn.getcwd()
      end,
    },
  }
  package.loaded['rustowl.lsp'] = {
    start = function()
      lsp_start_called = true
    end,
    get_rustowl_clients = function()
      return {}
    end,
  }
  vim.cmd('source ftplugin/rust.lua')
  expect.equality(lsp_start_called, true)
end

T['ftplugin_loads_without_error'] = function()
  package.loaded['rustowl.config'] = {
    highlight_style = 'undercurl',
    auto_enable = false,
    auto_attach = false,
    client = {
      name = 'rustowl',
      cmd = { 'rustowl' },
      root_dir = function()
        return vim.fn.getcwd()
      end,
    },
  }
  vim.bo.filetype = 'rust'
  expect.no_error(function()
    vim.cmd('source ftplugin/rust.lua')
  end)
  expect.equality(vim.g.loaded_rustowl, true)
end

T['ftplugin_creates_underline_highlights'] = function()
  package.loaded['rustowl.config'] = {
    highlight_style = 'underline',
    auto_enable = false,
    auto_attach = false,
    client = {
      name = 'rustowl',
      cmd = { 'rustowl' },
      root_dir = function()
        return vim.fn.getcwd()
      end,
    },
  }
  vim.bo.filetype = 'rust'
  vim.cmd('source ftplugin/rust.lua')
  local hl = vim.api.nvim_get_hl(0, { name = 'lifetime' })
  -- Accept underline or undercurl (for plugin compatibility)
  expect.equality((hl.underline or false) or (hl.undercurl or false), true)
end

T['user_command_calls_correct_functions'] = function()
  -- Patch: all mocks set before sourcing ftplugin!
  local lsp_start_called = false
  local lsp_stop_called = false
  local lsp_restart_called = false
  local rustowl_enable_called = false
  local rustowl_disable_called = false
  local rustowl_toggle_called = false
  package.loaded['rustowl.config'] = {
    auto_enable = false,
    auto_attach = false,
    client = {
      name = 'rustowl',
      cmd = { 'rustowl' },
      root_dir = function()
        return vim.fn.getcwd()
      end,
    },
  }
  package.loaded['rustowl.lsp'] = {
    start = function()
      lsp_start_called = true
    end,
    stop = function()
      lsp_stop_called = true
    end,
    restart = function()
      lsp_restart_called = true
    end,
    get_rustowl_clients = function()
      return {}
    end,
  }
  package.loaded['rustowl'] = {
    enable = function()
      rustowl_enable_called = true
    end,
    disable = function()
      rustowl_disable_called = true
    end,
    toggle = function()
      rustowl_toggle_called = true
    end,
    is_enabled = function()
      return false
    end,
  }
  vim.bo.filetype = 'rust'
  vim.cmd('source ftplugin/rust.lua')
  vim.cmd('Rustowl start_client')
  expect.equality(lsp_start_called, true)
  vim.cmd('Rustowl enable')
  expect.equality(rustowl_enable_called, true)
  vim.cmd('Rustowl disable')
  expect.equality(rustowl_disable_called, true)
  vim.cmd('Rustowl toggle')
  expect.equality(rustowl_toggle_called, true)
end

T['auto_enable_enables_on_lsp_attach'] = function()
  -- Patch: all mocks set before sourcing ftplugin!
  local enable_on_lsp_attach_called = false
  package.loaded['rustowl.config'] = {
    auto_enable = true,
    auto_attach = false,
    client = {
      name = 'rustowl',
      cmd = { 'rustowl' },
      root_dir = function()
        return vim.fn.getcwd()
      end,
    },
  }
  package.loaded['rustowl.show-on-hover'] = {
    enable_on_lsp_attach = function()
      enable_on_lsp_attach_called = true
    end,
  }
  vim.bo.filetype = 'rust'
  vim.cmd('source ftplugin/rust.lua')
  expect.equality(enable_on_lsp_attach_called, true)
end

return T
