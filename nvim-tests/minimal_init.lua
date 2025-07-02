vim.opt.rtp:append(vim.fn.getcwd())

if #vim.api.nvim_list_uis() == 0 then
  -- Add 'mini.nvim' to 'runtimepath'
  local path_package = vim.fn.stdpath('data') .. '/site/'
  local mini_path = path_package .. 'pack/deps/start/mini.nvim'
  
  if not vim.loop.fs_stat(mini_path) then
    vim.cmd('echo "Installing mini.nvim"')
    vim.fn.system({
      'git', 'clone', '--filter=blob:none',
      'https://github.com/echasnovski/mini.nvim', mini_path
    })
  end
  
  vim.cmd('set rtp+=' .. mini_path)
  require("mini.test").setup({
      collect = {
        find_files = function()
          return vim.fn.globpath('nvim-tests', '**/test_*.lua', true, true)
        end,
      },
  })
end
