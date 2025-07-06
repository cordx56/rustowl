-- Tests for the show-on-hover module (lua/rustowl/show-on-hover.lua)
local MiniTest = require('mini.test')
local expect = MiniTest.expect

local T = MiniTest.new_set {
  hooks = {
    pre_case = function()
      if vim.lsp and vim.lsp.config then
        vim.lsp.config.rustowl = {}
      end
      -- Create a test buffer
      vim.cmd('enew')
      vim.bo.filetype = 'rust'

      -- Clear any existing autogroups
      pcall(vim.api.nvim_del_augroup_by_name, 'RustOwl')
      pcall(vim.api.nvim_del_augroup_by_name, 'RustOwlLspAttach')

      package.loaded['rustowl.lsp'] = nil
      package.loaded['rustowl.config'] = nil
      package.loaded['rustowl.highlight'] = nil
      package.loaded['rustowl.show-on-hover'] = nil
    end,
    post_case = function()
      -- Clean up
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

-- Test module loading
T['can_require_show_on_hover_module'] = function()
  local show_on_hover = require('rustowl.show-on-hover')

  expect.equality(type(show_on_hover), 'table')
  expect.equality(type(show_on_hover.enable), 'function')
  expect.equality(type(show_on_hover.disable), 'function')
  expect.equality(type(show_on_hover.toggle), 'function')
  expect.equality(type(show_on_hover.is_enabled), 'function')
  expect.equality(type(show_on_hover.enable_on_lsp_attach), 'function')
end

-- Test is_enabled function initial state
T['is_enabled_initial_state'] = function()
  local show_on_hover = require('rustowl.show-on-hover')

  expect.equality(show_on_hover.is_enabled(), false)
end

-- Test enable_on_lsp_attach function
T['enable_on_lsp_attach_creates_autocmd'] = function()
  local show_on_hover = require('rustowl.show-on-hover')

  expect.no_error(function()
    show_on_hover.enable_on_lsp_attach()
  end)

  -- Check if autogroup was created
  local augroups = vim.api.nvim_get_autocmds { group = 'RustOwlLspAttach' }
  expect.equality(#augroups > 0, true)
end

-- Test enable function
T['enable_function_works'] = function()
  local show_on_hover = require('rustowl.show-on-hover')

  -- Mock dependencies
  package.loaded['rustowl.lsp'] = {
    get_rustowl_clients = function()
      return {}
    end,
    start = function() end,
  }

  package.loaded['rustowl.config'] = {
    idle_time = 100, -- Short idle time for testing
  }

  package.loaded['rustowl.highlight'] = {
    enable = function() end,
    disable = function() end,
  }

  local bufnr = vim.api.nvim_get_current_buf()

  expect.no_error(function()
    show_on_hover.enable(bufnr)
  end)

  expect.equality(show_on_hover.is_enabled(), true)

  -- Check if autocmds were created
  local autocmds = vim.api.nvim_get_autocmds { group = 'RustOwl' }
  expect.equality(#autocmds > 0, true)
end

-- Test disable function
T['disable_function_works'] = function()
  local show_on_hover = require('rustowl.show-on-hover')

  -- Mock dependencies
  package.loaded['rustowl.lsp'] = {
    get_rustowl_clients = function()
      return {}
    end,
    start = function() end,
  }

  package.loaded['rustowl.config'] = {
    idle_time = 100,
  }

  local highlight_disable_called = false
  package.loaded['rustowl.highlight'] = {
    enable = function() end,
    disable = function()
      highlight_disable_called = true
    end,
  }

  local bufnr = vim.api.nvim_get_current_buf()

  -- First enable to have something to disable
  show_on_hover.enable(bufnr)
  expect.equality(show_on_hover.is_enabled(), true)

  -- Now disable
  show_on_hover.disable(bufnr)

  expect.equality(show_on_hover.is_enabled(), false)
  expect.equality(highlight_disable_called, true)
end

-- Test toggle function
T['toggle_function_works'] = function()
  local show_on_hover = require('rustowl.show-on-hover')

  -- Mock dependencies
  package.loaded['rustowl.lsp'] = {
    get_rustowl_clients = function()
      return {}
    end,
    start = function() end,
  }

  package.loaded['rustowl.config'] = {
    idle_time = 100,
  }

  package.loaded['rustowl.highlight'] = {
    enable = function() end,
    disable = function() end,
  }

  local bufnr = vim.api.nvim_get_current_buf()

  -- Initially disabled
  expect.equality(show_on_hover.is_enabled(), false)

  -- Toggle should enable
  show_on_hover.toggle(bufnr)
  expect.equality(show_on_hover.is_enabled(), true)

  -- Toggle should disable
  show_on_hover.toggle(bufnr)
  expect.equality(show_on_hover.is_enabled(), false)
end

-- Test enable function starts LSP when no clients
T['enable_starts_lsp_when_no_clients'] = function()
  local show_on_hover = require('rustowl.show-on-hover')

  local lsp_start_called = false
  package.loaded['rustowl.lsp'] = {
    get_rustowl_clients = function()
      return {}
    end, -- No clients
    start = function()
      lsp_start_called = true
    end,
  }

  package.loaded['rustowl.config'] = {
    idle_time = 100,
  }

  package.loaded['rustowl.highlight'] = {
    enable = function() end,
    disable = function() end,
  }

  local bufnr = vim.api.nvim_get_current_buf()
  show_on_hover.enable(bufnr)

  expect.equality(lsp_start_called, true)
end

-- Test enable function with existing clients
T['enable_doesnt_start_lsp_when_clients_exist'] = function()
  local show_on_hover = require('rustowl.show-on-hover')

  local lsp_start_called = false
  package.loaded['rustowl.lsp'] = {
    get_rustowl_clients = function()
      return { { id = 1 } } -- Mock existing client
    end,
    start = function()
      lsp_start_called = true
    end,
  }

  package.loaded['rustowl.config'] = {
    idle_time = 100,
  }

  package.loaded['rustowl.highlight'] = {
    enable = function() end,
    disable = function() end,
  }

  local bufnr = vim.api.nvim_get_current_buf()
  show_on_hover.enable(bufnr)

  expect.equality(lsp_start_called, false)
end

return T
