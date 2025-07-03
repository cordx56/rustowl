-- Tests for the LSP module (lua/rustowl/lsp.lua)
local MiniTest = require('mini.test')
local expect = MiniTest.expect

local T = MiniTest.new_set {
  hooks = {
    pre_case = function()
      vim.g.rustowl = nil
      if vim.lsp and vim.lsp.config then
        vim.lsp.config.rustowl = {}
      end
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
    end,
  },
}

-- Test getting rustowl clients
T['get_rustowl_clients_function'] = function()
  local lsp = require('rustowl.lsp')

  expect.equality(type(lsp.get_rustowl_clients), 'function')

  -- Should return a table (empty list when no clients)
  local clients = lsp.get_rustowl_clients()
  expect.equality(type(clients), 'table')
end

-- Test get_rustowl_clients with filter
T['get_rustowl_clients_with_filter'] = function()
  local lsp = require('rustowl.lsp')

  local filter = { bufnr = vim.api.nvim_get_current_buf() }
  local clients = lsp.get_rustowl_clients(filter)
  expect.equality(type(clients), 'table')
end

-- Test start function
T['start_function_exists'] = function()
  local lsp = require('rustowl.lsp')

  expect.equality(type(lsp.start), 'function')
end

-- Test start function with no root_dir
T['start_function_handles_no_root_dir'] = function()
  local lsp = require('rustowl.lsp')

  -- Mock config to return nil root_dir
  package.loaded['rustowl.config'] = {
    client = {
      root_dir = function()
        return nil
      end,
    },
  }

  -- Capture vim.notify calls
  local original_notify = vim.notify
  vim.notify = function()
    -- Just a placeholder for the mock
  end

  local result = lsp.start()

  -- Wait for scheduled notification
  vim.cmd('doautocmd User')

  -- Restore original notify
  vim.notify = original_notify

  expect.equality(result, nil)
  -- Note: The notification is scheduled, so we might not catch it in this test
end

-- Test stop function
T['stop_function_exists'] = function()
  local lsp = require('rustowl.lsp')

  expect.equality(type(lsp.stop), 'function')
end

-- Test restart function
T['restart_function_exists'] = function()
  local lsp = require('rustowl.lsp')

  expect.equality(type(lsp.restart), 'function')
end

-- Test start function with valid root_dir
T['start_function_with_valid_root_dir'] = function()
  vim.g.rustowl = nil
  -- Provide a valid config for this test (to avoid config.lua validation error)
  package.loaded['rustowl.config'] = {
    auto_attach = true,
    auto_enable = false,
    idle_time = 500,
    highlight_style = 'undercurl',
    client = {
      name = 'rustowl',
      cmd = { 'rustowl' },
      root_dir = function()
        return '/tmp/test-project'
      end, -- valid!
    },
  }
  local lsp = require('rustowl.lsp')

  -- Mock vim.lsp.start to avoid actually starting LSP
  local lsp_start_called = false
  local lsp_start_config = nil
  local original_lsp_start = vim.lsp.start
  vim.lsp.start = function(config)
    lsp_start_called = true
    lsp_start_config = config
    return 1 -- Mock client ID
  end

  -- Mock vim.fs.root to return a valid root directory
  local original_fs_root = vim.fs.root
  vim.fs.root = function()
    return '/tmp/test-project'
  end

  local result = lsp.start()

  -- Restore original functions
  vim.lsp.start = original_lsp_start
  vim.fs.root = original_fs_root

  expect.equality(lsp_start_called, true)
  expect.equality(type(lsp_start_config), 'table')
  expect.equality(lsp_start_config.root_dir, '/tmp/test-project')
  expect.equality(lsp_start_config.name, 'rustowl')
  expect.equality(result, 1)
end

-- Test client notification compatibility
T['client_notify_compatibility'] = function()
  local lsp = require('rustowl.lsp')

  -- This test ensures the module loads without errors
  -- The actual client_notify function is internal and tested indirectly
  expect.equality(type(lsp), 'table')
end

return T
