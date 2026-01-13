use anyhow::{Context, Result, anyhow};
use clap::Parser;

use crate::util::{Cmd, ensure_tool, repo_root};

#[derive(Parser, Debug)]
#[command(
    about = "Run Neovim integration tests",
    long_about = "Runs the tests in `nvim-tests/` using a headless Neovim instance.

Requirements:
- `nvim` must be installed and discoverable on PATH.

This uses the MiniTest-based harness via `nvim-tests/minimal_init.lua`."
)]
pub struct Args {}

pub async fn run(_args: Args) -> Result<()> {
    ensure_tool("nvim")?;
    let root = repo_root()?;

    let output = Cmd::new("nvim")
        .args([
            "--headless",
            "--noplugin",
            "-u",
            "./nvim-tests/minimal_init.lua",
            "-c",
            "lua MiniTest.run()",
            "-c",
            "qa",
        ])
        .cwd(&root)
        .output()
        .await
        .context("run nvim tests")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    print!("{stdout}");
    eprint!("{stderr}");

    if output.status.success()
        && (stdout.contains("Fails (0) and Notes (0)")
            || stderr.contains("Fails (0) and Notes (0)"))
    {
        Ok(())
    } else {
        Err(anyhow!("neovim tests failed"))
    }
}
