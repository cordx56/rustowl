-- Tests for the main rustowl module (lua/rustowl/init.lua)
local MiniTest = require('mini.test')
local expect = MiniTest.expect

-- Create a test set
local T = MiniTest.new_set {
  hooks = {
    pre_case = function()
      -- Reset vim global variables before each test
      vim.g.rustowl = nil
      vim.g.loaded_rustowl = nil
      if vim.lsp and vim.lsp.config then
        vim.lsp.config.rustowl = {}
      end

      -- Clear any existing autocmds
      pcall(vim.api.nvim_del_augroup_by_name, 'RustOwl')
      pcall(vim.api.nvim_del_augroup_by_name, 'RustOwlLspAttach')

      -- Create a temporary buffer for testing
      vim.cmd('enew')
      vim.bo.filetype = 'rust'

      package.loaded['rustowl'] = nil
      package.loaded['rustowl.lsp'] = nil
      package.loaded['rustowl.config'] = nil
      package.loaded['rustowl.highlight'] = nil
      package.loaded['rustowl.show-on-hover'] = nil
    end,
    post_case = function()
      -- Clean up after each test
      vim.cmd('bwipe!')

      package.loaded['rustowl'] = nil
      package.loaded['rustowl.lsp'] = nil
      package.loaded['rustowl.config'] = nil
      package.loaded['rustowl.highlight'] = nil
      package.loaded['rustowl.show-on-hover'] = nil
    end,
  },
}

-- Test module loading
T['can_require_rustowl'] = function()
  local rustowl = require('rustowl')
  expect.equality(type(rustowl), 'table')
  expect.equality(type(rustowl.setup), 'function')
  expect.equality(type(rustowl.enable), 'function')
  expect.equality(type(rustowl.disable), 'function')
  expect.equality(type(rustowl.toggle), 'function')
  expect.equality(type(rustowl.is_enabled), 'function')
end

-- Test setup function
T['setup_function_works'] = function()
  local rustowl = require('rustowl')

  -- Test with empty options
  rustowl.setup()
  expect.equality(vim.g.rustowl, nil)

  -- Test with custom options
  local opts = {
    auto_attach = false,
    auto_enable = true,
    idle_time = 1000,
  }
  rustowl.setup(opts)
  expect.equality(vim.g.rustowl, opts)
end

-- Test enable function
T['enable_function_calls_show_on_hover'] = function()
  local rustowl = require('rustowl')

  -- Mock the show-on-hover module
  local show_on_hover_enable_called = false
  local show_on_hover_enable_bufnr = nil

  package.loaded['rustowl.show-on-hover'] = {
    enable = function(bufnr)
      show_on_hover_enable_called = true
      show_on_hover_enable_bufnr = bufnr
    end,
    disable = function() end,
    toggle = function() end,
    is_enabled = function()
      return false
    end,
  }

  rustowl.enable(123)

  expect.equality(show_on_hover_enable_called, true)
  expect.equality(show_on_hover_enable_bufnr, 123)
end

-- Test disable function
T['disable_function_calls_show_on_hover'] = function()
  local rustowl = require('rustowl')

  -- Mock the show-on-hover module
  local show_on_hover_disable_called = false
  local show_on_hover_disable_bufnr = nil

  package.loaded['rustowl.show-on-hover'] = {
    enable = function() end,
    disable = function(bufnr)
      show_on_hover_disable_called = true
      show_on_hover_disable_bufnr = bufnr
    end,
    toggle = function() end,
    is_enabled = function()
      return true
    end,
  }

  rustowl.disable(456)

  expect.equality(show_on_hover_disable_called, true)
  expect.equality(show_on_hover_disable_bufnr, 456)
end

-- Test toggle function
T['toggle_function_calls_show_on_hover'] = function()
  local rustowl = require('rustowl')

  -- Mock the show-on-hover module
  local show_on_hover_toggle_called = false
  local show_on_hover_toggle_bufnr = nil

  package.loaded['rustowl.show-on-hover'] = {
    enable = function() end,
    disable = function() end,
    toggle = function(bufnr)
      show_on_hover_toggle_called = true
      show_on_hover_toggle_bufnr = bufnr
    end,
    is_enabled = function()
      return false
    end,
  }

  rustowl.toggle(789)

  expect.equality(show_on_hover_toggle_called, true)
  expect.equality(show_on_hover_toggle_bufnr, 789)
end

-- Test is_enabled function
T['is_enabled_function_returns_correct_state'] = function()
  local rustowl = require('rustowl')

  -- Mock enabled state
  package.loaded['rustowl.show-on-hover'] = {
    enable = function() end,
    disable = function() end,
    toggle = function() end,
    is_enabled = function()
      return true
    end,
  }

  expect.equality(rustowl.is_enabled(), true)

  -- Mock disabled state
  package.loaded['rustowl.show-on-hover'] = {
    enable = function() end,
    disable = function() end,
    toggle = function() end,
    is_enabled = function()
      return false
    end,
  }

  expect.equality(rustowl.is_enabled(), false)
end

return T
