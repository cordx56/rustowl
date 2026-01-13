use anyhow::{Context, Result, anyhow};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::Command;

pub fn repo_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir().context("get current dir")?;
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("crates").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    Err(anyhow!("could not locate repo root"))
}

pub fn is_ci() -> bool {
    std::env::var_os("CI").is_some() || std::env::var_os("GITHUB_ACTIONS").is_some()
}

pub fn which<S: AsRef<OsStr>>(tool: S) -> Option<PathBuf> {
    let tool = tool.as_ref();
    let paths = std::env::var_os("PATH")?;
    for path in std::env::split_paths(&paths) {
        let candidate = path.join(tool);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate_exe = path.join(format!("{}.exe", tool.to_string_lossy()));
            if candidate_exe.is_file() {
                return Some(candidate_exe);
            }
        }
    }
    None
}

#[derive(Clone, Debug)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

impl Cmd {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub async fn run(self) -> Result<()> {
        run_cmd(self, false).await
    }

    pub async fn output(self) -> Result<std::process::Output> {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::null());
        cmd.output()
            .await
            .with_context(|| format!("run {}", display_cmd(&self.program, &self.args)))
    }
}

async fn run_cmd(cmd: Cmd, quiet: bool) -> Result<()> {
    let mut c = Command::new(&cmd.program);
    c.args(&cmd.args);
    if let Some(cwd) = &cmd.cwd {
        c.current_dir(cwd);
    }
    for (k, v) in &cmd.env {
        c.env(k, v);
    }
    c.stdin(Stdio::null());

    if quiet {
        c.stdout(Stdio::null());
        c.stderr(Stdio::null());
    } else {
        c.stdout(Stdio::inherit());
        c.stderr(Stdio::inherit());
    }

    let status = c
        .status()
        .await
        .with_context(|| format!("run {}", display_cmd(&cmd.program, &cmd.args)))?;
    if !status.success() {
        return Err(anyhow!(
            "command failed ({}): {}",
            status,
            display_cmd(&cmd.program, &cmd.args)
        ));
    }
    Ok(())
}

pub fn display_cmd(program: &str, args: &[String]) -> String {
    let mut s = program.to_string();
    for a in args {
        s.push(' ');
        s.push_str(&shell_escape(a));
    }
    s
}

fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._/:".contains(c))
    {
        return s.to_string();
    }
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

pub fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
    std::fs::read_to_string(&path).with_context(|| format!("read {}", path.as_ref().display()))
}

pub fn write_string(path: impl AsRef<Path>, contents: &str) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    std::fs::write(&path, contents).with_context(|| format!("write {}", path.as_ref().display()))
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2}GiB", b / GB)
    } else if b >= MB {
        format!("{:.2}MiB", b / MB)
    } else if b >= KB {
        format!("{:.2}KiB", b / KB)
    } else {
        format!("{}B", bytes)
    }
}

pub fn percent_change(baseline: f64, current: f64) -> Option<f64> {
    if baseline == 0.0 {
        return None;
    }
    Some(((current - baseline) / baseline) * 100.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsKind {
    Linux,
    Macos,
    Windows,
    Other,
}

pub fn os_kind() -> OsKind {
    if cfg!(target_os = "linux") {
        OsKind::Linux
    } else if cfg!(target_os = "macos") {
        OsKind::Macos
    } else if cfg!(target_os = "windows") {
        OsKind::Windows
    } else {
        OsKind::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Dnf,
    Yum,
    Pacman,
    Brew,
}

pub fn detect_package_manager() -> Option<PackageManager> {
    if which("apt-get").is_some() {
        return Some(PackageManager::Apt);
    }
    if which("dnf").is_some() {
        return Some(PackageManager::Dnf);
    }
    if which("yum").is_some() {
        return Some(PackageManager::Yum);
    }
    if which("pacman").is_some() {
        return Some(PackageManager::Pacman);
    }
    if which("brew").is_some() {
        return Some(PackageManager::Brew);
    }
    None
}

pub async fn sudo_install(pkgs: &[&str]) -> Result<()> {
    let mgr =
        detect_package_manager().ok_or_else(|| anyhow!("no supported package manager found"))?;
    match mgr {
        PackageManager::Apt => {
            Cmd::new("sudo").args(["apt-get", "update"]).run().await?;
            Cmd::new("sudo")
                .args(["apt-get", "install", "-y"])
                .args(pkgs.iter().copied())
                .run()
                .await
        }
        PackageManager::Dnf => {
            Cmd::new("sudo")
                .args(["dnf", "install", "-y"])
                .args(pkgs.iter().copied())
                .run()
                .await
        }
        PackageManager::Yum => {
            Cmd::new("sudo")
                .args(["yum", "install", "-y"])
                .args(pkgs.iter().copied())
                .run()
                .await
        }
        PackageManager::Pacman => {
            Cmd::new("sudo")
                .args(["pacman", "-S", "--noconfirm"])
                .args(pkgs.iter().copied())
                .run()
                .await
        }
        PackageManager::Brew => {
            Cmd::new("brew")
                .args(["install"])
                .args(pkgs.iter().copied())
                .run()
                .await
        }
    }
}

pub fn ensure_tool(tool: &str) -> Result<()> {
    if which(tool).is_none() {
        return Err(anyhow!("required tool not found in PATH: {tool}"));
    }
    Ok(())
}
