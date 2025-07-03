local MiniTest = require('mini.test')
local expect = MiniTest.expect

local T = MiniTest.new_set {
  hooks = {
    pre_case = function()
      if vim.lsp and vim.lsp.config then
        vim.lsp.config.rustowl = {}
      end
      vim.cmd('enew')
      vim.bo.filetype = 'rust'
      vim.api.nvim_buf_set_lines(0, 0, -1, false, {
        'fn main() {',
        '    let x = 5;',
        '    println!("{}", x);',
        '}',
      })
      package.loaded['rustowl.lsp'] = nil
      package.loaded['rustowl.config'] = nil
      package.loaded['rustowl.highlight'] = nil
      package.loaded['rustowl.show-on-hover'] = nil
    end,
    post_case = function()
      vim.cmd('bwipe!')
      package.loaded['rustowl.lsp'] = nil
      package.loaded['rustowl.config'] = nil
      package.loaded['rustowl.highlight'] = nil
      package.loaded['rustowl.show-on-hover'] = nil
    end,
  },
}

T['can_require_highlight_module'] = function()
  local highlight = require('rustowl.highlight')
  expect.equality(type(highlight), 'table')
  expect.equality(type(highlight.enable), 'function')
  expect.equality(type(highlight.disable), 'function')
end

T['enable_function_accepts_parameters'] = function()
  local highlight = require('rustowl.highlight')
  package.loaded['rustowl.lsp'] = {
    get_rustowl_clients = function()
      return {}
    end,
  }
  expect.no_error(function()
    highlight.enable(1, 0, vim.api.nvim_get_current_buf())
  end)
end

T['enable_function_with_mock_client'] = function()
  -- Mock is both callable and has .request method to match any plugin usage
  local request_called = false
  local request_params = nil
  local request_method = nil

  local mock_client = {}
  function mock_client:request(method, params, callback, _)
    request_called = true
    request_method = method
    request_params = params
    local result = {
      decorations = {
        {
          type = 'lifetime',
          range = {
            start = { line = 0, character = 0 },
            ['end'] = { line = 0, character = 5 },
          },
          overlapped = false,
        },
      },
    }
    if type(callback) == 'function' then
      callback(nil, result, nil)
    end
  end
  setmetatable(mock_client, {
    __call = function(_, method, params, callback, _)
      request_called = true
      request_method = method
      request_params = params
      local result = {
        decorations = {
          {
            type = 'lifetime',
            range = {
              start = { line = 0, character = 0 },
              ['end'] = { line = 0, character = 5 },
            },
            overlapped = false,
          },
        },
      }
      if type(callback) == 'function' then
        callback(nil, result, nil)
      end
    end,
  })

  package.loaded['rustowl.lsp'] = {
    get_rustowl_clients = function()
      return { mock_client }
    end,
  }
  local highlight = require('rustowl.highlight')
  highlight.enable(1, 0, vim.api.nvim_get_current_buf())

  expect.equality(request_called, true)
  expect.equality(request_method, 'rustowl/cursor')
  expect.equality(type(request_params), 'table')
  expect.equality(request_params.position.line, 0)
  expect.equality(request_params.position.character, 0)
end

T['disable_function_clears_highlights'] = function()
  local highlight = require('rustowl.highlight')
  local bufnr = vim.api.nvim_get_current_buf()
  expect.no_error(function()
    highlight.disable(bufnr)
  end)
  expect.no_error(function()
    highlight.disable(nil)
  end)
end

T['highlight_namespace_exists'] = function()
  require('rustowl.highlight')
  local namespaces = vim.api.nvim_get_namespaces()
  expect.equality(type(namespaces.rustowl), 'number')
end

T['enable_function_ignores_overlapped_decorations'] = function()
  local highlight = require('rustowl.highlight')
  local highlight_range_called = false
  local original_highlight_range = vim.highlight.range
  vim.highlight.range = function()
    highlight_range_called = true
  end
  local mock_client = {
    request = function(callback)
      local result = {
        decorations = {
          {
            type = 'lifetime',
            range = {
              start = { line = 0, character = 0 },
              ['end'] = { line = 0, character = 5 },
            },
            overlapped = true,
          },
        },
      }
      if type(callback) == 'function' then
        callback(nil, result, nil)
      end
    end,
  }
  package.loaded['rustowl.lsp'] = {
    get_rustowl_clients = function()
      return { mock_client }
    end,
  }
  highlight = require('rustowl.highlight')
  highlight.enable(1, 0, vim.api.nvim_get_current_buf())
  vim.highlight.range = original_highlight_range
  expect.equality(highlight_range_called, false)
end

return T
