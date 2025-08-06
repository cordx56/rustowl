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
  vim.lsp.config.rustowl = {}
end

local T = MiniTest.new_set {
  hooks = {
    pre_case = function()
      -- Reset vim.g.rustowl before each test
      vim.g.rustowl = nil
      -- Reload the config module to get fresh state
      package.loaded['rustowl.config'] = nil
      monkeypatch_vim_lsp_config()
    end,
  },
}

T['default colors'] = function()
  vim.g.rustowl = {}
  local config = require('rustowl.config')

  expect.equality(config.colors.lifetime, '#00cc00')
  expect.equality(config.colors.imm_borrow, '#0000cc')
  expect.equality(config.colors.mut_borrow, '#cc00cc')
  expect.equality(config.colors.move, '#cccc00')
  expect.equality(config.colors.call, '#cccc00')
  expect.equality(config.colors.outlive, '#cc0000')
end

T['custom colors'] = function()
  vim.g.rustowl = {
    colors = {
      lifetime = '#32cd32',
      imm_borrow = '#4169e1',
      mut_borrow = '#ff69b4',
      move = '#ffa500',
      call = '#ffd700',
      outlive = '#dc143c',
    },
  }
  local config = require('rustowl.config')

  expect.equality(config.colors.lifetime, '#32cd32')
  expect.equality(config.colors.imm_borrow, '#4169e1')
  expect.equality(config.colors.mut_borrow, '#ff69b4')
  expect.equality(config.colors.move, '#ffa500')
  expect.equality(config.colors.call, '#ffd700')
  expect.equality(config.colors.outlive, '#dc143c')
end

T['partial color customization'] = function()
  vim.g.rustowl = {
    colors = {
      lifetime = '#90ee90',
      outlive = '#ff4500',
      -- Other colors should use defaults
    },
  }
  local config = require('rustowl.config')

  expect.equality(config.colors.lifetime, '#90ee90')
  expect.equality(config.colors.imm_borrow, '#0000cc') -- default
  expect.equality(config.colors.mut_borrow, '#cc00cc') -- default
  expect.equality(config.colors.move, '#cccc00') -- default
  expect.equality(config.colors.call, '#cccc00') -- default
  expect.equality(config.colors.outlive, '#ff4500')
end

return T
