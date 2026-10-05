use std::env;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::fs::{create_dir_all, read_to_string, remove_dir_all, rename};

use flate2::read::GzDecoder;
use tar::Archive;

pub const TOOLCHAIN: &str = env!("RUSTOWL_TOOLCHAIN");
pub const HOST_TUPLE: &str = env!("HOST_TUPLE");
const TOOLCHAIN_CHANNEL: &str = env!("TOOLCHAIN_CHANNEL");
const TOOLCHAIN_DATE: Option<&str> = option_env!("TOOLCHAIN_DATE");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolchainError(pub &'static str);

impl std::fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ToolchainError {}

pub static FALLBACK_RUNTIME_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let opt = PathBuf::from("/opt/rustowl");
    if sysroot_from_runtime(&opt).is_dir() {
        return opt;
    }
    let same = env::current_exe().unwrap().parent().unwrap().to_path_buf();
    if sysroot_from_runtime(&same).is_dir() {
        return same;
    }
    env::home_dir().unwrap().join(".rustowl")
});

fn recursive_read_dir(path: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if path.as_ref().is_dir() {
        for entry in read_dir(&path).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                paths.extend_from_slice(&recursive_read_dir(&path));
            } else {
                paths.push(path);
            }
        }
    }
    paths
}

pub fn sysroot_from_runtime(runtime: impl AsRef<Path>) -> PathBuf {
    runtime.as_ref().join("sysroot").join(TOOLCHAIN)
}

async fn get_runtime_dir() -> PathBuf {
    let sysroot = sysroot_from_runtime(&*FALLBACK_RUNTIME_DIR);
    if FALLBACK_RUNTIME_DIR.is_dir() && sysroot.is_dir() {
        return FALLBACK_RUNTIME_DIR.clone();
    }

    tracing::info!("sysroot not found; start setup toolchain");
    if let Err(e) = setup_toolchain(&*FALLBACK_RUNTIME_DIR, false).await {
        tracing::error!("{e:?}");
        std::process::exit(1);
    } else {
        FALLBACK_RUNTIME_DIR.clone()
    }
}

pub async fn get_sysroot() -> PathBuf {
    sysroot_from_runtime(get_runtime_dir().await)
}

fn progress_bar_style() -> Result<indicatif::ProgressStyle, ToolchainError> {
    use indicatif::*;
    Ok(
        ProgressStyle::with_template("{spinner:.green} {msg:<10} [{bar:30.cyan/blue}]  {pos:>3}%")
            .map_err(|_| {
                tracing::error!("failed to setup progress bar");
                ToolchainError("failed to setup progress bar")
            })?
            .progress_chars("#>-"),
    )
}

async fn download(url: &str, set_progress: impl Fn(usize)) -> Result<Vec<u8>, ToolchainError> {
    tracing::debug!("start downloading {url}...");
    let mut resp = match reqwest::get(url).await.and_then(|v| v.error_for_status()) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to download tarball");
            tracing::error!("{e:?}");
            return Err(ToolchainError("failed to download tarball"));
        }
    };

    let content_length = resp.content_length().unwrap_or(200_000_000) as usize;
    let mut data = Vec::with_capacity(content_length);
    let mut received = 0;
    while let Some(chunk) = match resp.chunk().await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to download runtime archive");
            tracing::error!("{e:?}");
            return Err(ToolchainError("failed to download runtime archive"));
        }
    } {
        data.extend_from_slice(&chunk);
        let current = data.len() * 100 / content_length;
        if received != current {
            set_progress(current);
            tracing::debug!("received from {url}: {current:3}%");
            received = current;
        }
    }
    tracing::debug!("download finished");
    Ok(data)
}
async fn download_tarball_and_extract(
    url: &str,
    dest: &Path,
    set_progress: impl Fn(usize),
) -> Result<(), ToolchainError> {
    let data = download(url, set_progress).await?;
    let decoder = GzDecoder::new(&*data);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest).map_err(|_| {
        tracing::error!("failed to unpack tarball");
        ToolchainError("failed to unpack tarball")
    })?;
    tracing::debug!("successfully unpacked");
    Ok(())
}
#[cfg(target_os = "windows")]
async fn download_zip_and_extract(
    url: &str,
    dest: &Path,
    set_progress: impl Fn(usize),
) -> Result<(), ToolchainError> {
    use zip::ZipArchive;
    let data = download(url, set_progress).await?;
    let cursor = std::io::Cursor::new(&*data);

    let mut archive = match ZipArchive::new(cursor) {
        Ok(archive) => archive,
        Err(e) => {
            tracing::error!("failed to read ZIP archive");
            tracing::error!("{e:?}");
            return Err(ToolchainError("failed to read ZIP archive"));
        }
    };
    archive.extract(dest).map_err(|e| {
        tracing::error!("failed to unpack zip: {e}");
        ToolchainError("failed to unpack zip")
    })?;
    tracing::debug!("successfully unpacked");
    Ok(())
}

async fn install_components(
    components: impl IntoIterator<Item = impl AsRef<str>>,
    dest: PathBuf,
) -> Result<(), ToolchainError> {
    use indicatif::*;
    let m = MultiProgress::new();

    let components: Vec<_> = components
        .into_iter()
        .map(|s| s.as_ref().to_owned())
        .collect();
    let mut threads = Vec::with_capacity(components.len());
    for component in components {
        let pb = m.add(ProgressBar::new(100));
        pb.set_style(progress_bar_style()?);
        pb.set_message(component.clone());

        let dest = dest.clone();
        let handle = tokio::spawn(async move {
            let tempdir =
                tempfile::tempdir().map_err(|_| ToolchainError("failed to create temp dir"))?;
            // Using `tempdir.path()` more than once causes SEGV, so we use `tempdir.path().to_owned()`.
            let temp_path = tempdir.path().to_owned();
            tracing::debug!("temp dir is made: {}", temp_path.display());

            let dist_base = "https://static.rust-lang.org/dist";
            let base_url = match TOOLCHAIN_DATE {
                Some(v) => format!("{dist_base}/{v}"),
                None => dist_base.to_owned(),
            };

            let component_toolchain = format!("{component}-{TOOLCHAIN_CHANNEL}-{HOST_TUPLE}");
            let tarball_url = format!("{base_url}/{component_toolchain}.tar.gz");

            download_tarball_and_extract(&tarball_url, &temp_path, |v| pb.set_position(v as u64))
                .await?;

            let extracted_path = temp_path.join(&component_toolchain);
            let components = read_to_string(extracted_path.join("components"))
                .await
                .map_err(|_| {
                    tracing::error!("failed to read components list");
                    ToolchainError("failed to read components list")
                })?;
            let components = components.split_whitespace();

            for component in components {
                let component_path = extracted_path.join(component);
                for from in recursive_read_dir(&component_path) {
                    let rel_path = match from.strip_prefix(&component_path) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::error!("path error: {e}");
                            return Err(ToolchainError("path error"));
                        }
                    };
                    let to = dest.join(rel_path);
                    if let Err(e) = create_dir_all(to.parent().unwrap()).await {
                        tracing::error!("failed to create dir: {e}");
                        return Err(ToolchainError("failed to create dir"));
                    }
                    if let Err(e) = rename(&from, &to).await {
                        tracing::debug!("rename failed ({e}), falling back to copy and delete");
                        if let Err(copy_err) = tokio::fs::copy(&from, &to).await {
                            tracing::error!("file copy error (after rename failure): {copy_err}");
                            return Err(ToolchainError("file copy error"));
                        }
                        if let Err(del_err) = tokio::fs::remove_file(&from).await {
                            tracing::error!("file delete error (after copy): {del_err}");
                            return Err(ToolchainError("file delete error"));
                        }
                    }
                }
                tracing::debug!("component {component} successfully installed");
            }
            pb.finish_and_clear();
            Ok(())
        });
        threads.push(handle);
    }
    for thread in threads {
        if let Ok(res) = thread.await {
            if res.is_err() {
                tracing::error!("failed to install component")
            }
        } else {
            tracing::error!("failed to join component installation task");
        }
    }
    Ok(())
}
pub async fn setup_toolchain(
    dest: impl AsRef<Path>,
    skip_rustowl: bool,
) -> Result<(), ToolchainError> {
    setup_rust_toolchain(&dest).await?;
    if !skip_rustowl {
        setup_rustowl_toolchain(&dest).await?;
    }
    Ok(())
}
pub async fn setup_rust_toolchain(dest: impl AsRef<Path>) -> Result<(), ToolchainError> {
    let sysroot = sysroot_from_runtime(dest.as_ref());
    if create_dir_all(&sysroot).await.is_err() {
        tracing::error!("failed to create toolchain directory");
        return Err(ToolchainError("failed to create toolchain directory"));
    }

    tracing::info!("start installing Rust toolchain...");
    install_components(&["rustc", "rust-std", "cargo"], sysroot).await?;
    tracing::info!("installing Rust toolchain finished");
    Ok(())
}
pub async fn setup_rustowl_toolchain(dest: impl AsRef<Path>) -> Result<(), ToolchainError> {
    let pb = indicatif::ProgressBar::new(100);
    pb.set_style(progress_bar_style()?);

    tracing::info!("start installing RustOwl toolchain...");
    #[cfg(not(target_os = "windows"))]
    let rustowl_toolchain_result = {
        let rustowl_tarball_url = format!(
            "https://github.com/cordx56/rustowl/releases/download/v{}/rustowl-{HOST_TUPLE}.tar.gz",
            clap::crate_version!(),
        );
        download_tarball_and_extract(&rustowl_tarball_url, dest.as_ref(), |v| {
            pb.set_position(v as u64)
        })
        .await
    };
    #[cfg(target_os = "windows")]
    let rustowl_toolchain_result = {
        let rustowl_zip_url = format!(
            "https://github.com/cordx56/rustowl/releases/download/v{}/rustowl-{HOST_TUPLE}.zip",
            clap::crate_version!(),
        );
        download_zip_and_extract(&rustowl_zip_url, dest.as_ref(), |v| {
            pb.set_position(v as u64)
        })
        .await
    };
    pb.finish_and_clear();
    if rustowl_toolchain_result.is_ok() {
        tracing::info!("installing RustOwl toolchain finished");
    } else {
        tracing::warn!(
            "could not install RustOwl toolchain; local installed rustowlc will be used"
        );
    }
    Ok(())
}

pub async fn uninstall_toolchain() {
    let sysroot = sysroot_from_runtime(&*FALLBACK_RUNTIME_DIR);
    if sysroot.is_dir() {
        tracing::info!("remove sysroot: {}", sysroot.display());
        remove_dir_all(&sysroot).await.unwrap();
    }
}

pub async fn get_executable_path(name: &str) -> String {
    #[cfg(not(windows))]
    let exec_name = name.to_owned();
    #[cfg(windows)]
    let exec_name = format!("{name}.exe");

    let runtime_dir = get_runtime_dir().await;
    let exec_root = runtime_dir.join(&exec_name);
    if exec_root.is_file() {
        tracing::debug!("{name} is selected in runtime root");
        return exec_root.to_string_lossy().to_string();
    }

    let sysroot = get_sysroot().await;
    let exec_bin = sysroot.join("bin").join(&exec_name);
    if exec_bin.is_file() {
        tracing::debug!("{name} is selected in sysroot/bin");
        return exec_bin.to_string_lossy().to_string();
    }

    let mut current_exec = env::current_exe().unwrap();
    current_exec.set_file_name(&exec_name);
    if current_exec.is_file() {
        tracing::debug!("{name} is selected in the same directory as rustowl executable");
        return current_exec.to_string_lossy().to_string();
    }

    tracing::warn!("{name} not found; fallback");
    exec_name.to_owned()
}

pub async fn setup_cargo_command() -> tokio::process::Command {
    let cargo = get_executable_path("cargo").await;
    let mut command = tokio::process::Command::new(&cargo);
    let rustowlc = get_executable_path("rustowlc").await;

    // check user set flags
    let delimiter = 0x1f as char;
    let rustflags = env::var("RUSTFLAGS")
        .unwrap_or("".to_string())
        .split_whitespace()
        .fold("".to_string(), |acc, x| format!("{acc}{delimiter}{x}"));
    let encoded_flags = env::var("CARGO_ENCODED_RUSTFLAGS")
        .map(|v| format!("{v}{delimiter}"))
        .unwrap_or("".to_string());

    let sysroot = get_sysroot().await;
    // use `RUSTOWLC` and `RUSTOWLC_WORKSPACE_WRAPPER` env var to configure `rustowlc` path
    let rustowlc = env::var("RUSTOWLC").unwrap_or(rustowlc);
    let rustowlc_workspace = env::var("RUSTOWLC_WORKSPACE_WRAPPER").unwrap_or(rustowlc.clone());
    command
        .env("RUSTC", &rustowlc)
        .env("RUSTC_WORKSPACE_WRAPPER", &rustowlc_workspace)
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            format!(
                "{}--sysroot={}{}",
                encoded_flags,
                sysroot.display(),
                rustflags
            ),
        );
    set_rustc_env(&mut command, &sysroot);
    command
}

pub fn set_rustc_env(command: &mut tokio::process::Command, sysroot: &Path) {
    command.env("RUSTC_BOOTSTRAP", "1"); // Support nightly projects
    let log_filter = env::var("RUST_LOG")
        .unwrap_or_else(|_| crate::log_filter_directive(crate::effective_log_level()));
    command.env("RUST_LOG", log_filter);

    #[cfg(target_os = "linux")]
    {
        let mut paths = env::split_paths(&env::var("LD_LIBRARY_PATH").unwrap_or("".to_owned()))
            .collect::<std::collections::VecDeque<_>>();
        paths.push_front(sysroot.join("lib"));
        let paths = env::join_paths(paths).unwrap();
        command.env("LD_LIBRARY_PATH", paths);
    }
    #[cfg(target_os = "macos")]
    {
        let mut paths =
            env::split_paths(&env::var("DYLD_FALLBACK_LIBRARY_PATH").unwrap_or("".to_owned()))
                .collect::<std::collections::VecDeque<_>>();
        paths.push_front(sysroot.join("lib"));
        let paths = env::join_paths(paths).unwrap();
        command.env("DYLD_FALLBACK_LIBRARY_PATH", paths);
    }
    #[cfg(target_os = "windows")]
    {
        let mut paths = env::split_paths(&env::var_os("Path").unwrap())
            .collect::<std::collections::VecDeque<_>>();
        paths.push_front(sysroot.join("bin"));
        let paths = env::join_paths(paths).unwrap();
        command.env("Path", paths);
    }
}
