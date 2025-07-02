local MiniTest = require('mini.test')
local expect = MiniTest.expect

local function monkeypatch_vim_lsp_config()
  -- Patch: ensure vim.lsp and vim.lsp.config exist and are tables, and rustowl is a table
  if not vim.lsp then
    vim.lsp = {}
  end
  if not vim.lsp.config then
    vim.lsp.config = {}
  end
  -- This is what the config module wants!
  if vim.g.rustowl_as_lsp_config then
    vim.lsp.config.rustowl = vim.g.rustowl_as_lsp_config
  else
    vim.lsp.config.rustowl = {}
  end
end

local T = MiniTest.new_set {
  hooks = {
    pre_case = function()
      vim.g.rustowl = nil
      vim.g.rustowl_as_lsp_config = nil
      monkeypatch_vim_lsp_config()
      package.loaded['rustowl.lsp'] = nil
      package.loaded['rustowl.config'] = nil
      package.loaded['rustowl.highlight'] = nil
      package.loaded['rustowl.show-on-hover'] = nil
    end,
    post_case = function()
      package.loaded['rustowl.lsp'] = nil
      package.loaded['rustowl.config'] = nil
      package.loaded['rustowl.highlight'] = nil
      package.loaded['rustowl.show-on-hover'] = nil
      vim.g.rustowl = nil
      vim.g.rustowl_as_lsp_config = nil
    end,
  },
}

T['default_config_values'] = function()
  vim.g.rustowl_as_lsp_config = nil
  monkeypatch_vim_lsp_config()
  local config = require('rustowl.config')
  expect.equality(config.auto_attach, true)
  expect.equality(config.auto_enable, false)
  expect.equality(config.idle_time, 500)
  expect.equality(config.highlight_style, 'undercurl')
  expect.equality(type(config.client), 'table')
  expect.equality(config.client.name, 'rustowl')
  expect.equality(type(config.client.cmd), 'table')
  expect.equality(config.client.cmd[1], 'rustowl')
  expect.equality(type(config.client.root_dir), 'function')
end

T['user_config_override'] = function()
  vim.g.rustowl_as_lsp_config = {
    auto_attach = false,
    auto_enable = true,
    idle_time = 1000,
    highlight_style = 'underline',
  }
  monkeypatch_vim_lsp_config()
  local config = require('rustowl.config')
  expect.equality(config.auto_attach, false)
  expect.equality(config.auto_enable, true)
  expect.equality(config.idle_time, 1000)
  expect.equality(config.highlight_style, 'underline')
end

T['function_based_user_config'] = function()
  vim.g.rustowl_as_lsp_config = (function()
    return {
      auto_attach = false,
      idle_time = 2000,
    }
  end)()
  monkeypatch_vim_lsp_config()
  local config = require('rustowl.config')
  expect.equality(config.auto_attach, false)
  expect.equality(config.idle_time, 2000)
  expect.equality(config.auto_enable, false)
end

T['lsp_config_override'] = function()
  vim.g.rustowl_as_lsp_config = {
    auto_attach = false,
    client = {
      cmd = { 'custom-rustowl' },
    },
  }
  monkeypatch_vim_lsp_config()
  local config = require('rustowl.config')
  expect.equality(config.auto_attach, false)
  expect.equality(config.client.cmd[1], 'custom-rustowl')
end

T['invalid_highlight_style_warning'] = function()
  vim.g.rustowl_as_lsp_config = { highlight_style = 'invalid_style' }
  monkeypatch_vim_lsp_config()
  local notify_called = false
  local notify_message = nil
  local notify_level = nil
  local original_notify = vim.notify
  vim.notify = function(msg, level)
    notify_called = true
    notify_message = msg
    notify_level = level
  end
  local config = require('rustowl.config')
  vim.notify = original_notify
  expect.equality(notify_called, true)
  expect.equality(type(notify_message), 'string')
  expect.equality(notify_level, vim.log.levels.WARN)
  expect.equality(config.highlight_style, 'undercurl')
end

T['root_dir_function_works'] = function()
  vim.g.rustowl_as_lsp_config = {}
  monkeypatch_vim_lsp_config()
  local config = require('rustowl.config')
  expect.equality(type(config.client.root_dir), 'function')
  local result = config.client.root_dir()
  expect.equality(type(result) == 'string' or type(result) == 'nil', true)
end

T['config_validation'] = function()
  vim.g.rustowl_as_lsp_config = {
    auto_attach = 'not_boolean',
    auto_enable = true,
    idle_time = 500,
  }
  monkeypatch_vim_lsp_config()
  expect.error(function()
    require('rustowl.config')
  end)
end

return T
