use std::env;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use tokio::{
    fs::{create_dir_all, read_to_string, remove_dir_all, rename},
    process,
};

use flate2::read::GzDecoder;
use tar::Archive;

pub const TOOLCHAIN: &str = env!("RUSTOWL_TOOLCHAIN");
const HOST_TUPLE: &str = env!("HOST_TUPLE");
const TOOLCHAIN_CHANNEL: &str = env!("TOOLCHAIN_CHANNEL");
const TOOLCHAIN_DATE: Option<&str> = option_env!("TOOLCHAIN_DATE");

pub static FALLBACK_RUNTIME_DIRS: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
    let exec_dir = env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let cache_dir = env::var("HOME").map(|v| PathBuf::from(v).join(".cache").join("rustowl"));
    let mut dirs = Vec::with_capacity(3);
    if let Ok(cache_dir) = cache_dir {
        dirs.push(cache_dir);
    }
    dirs.push(exec_dir.join("rustowl-runtime"));
    dirs.push(exec_dir);
    dirs
});

const BUILD_RUNTIME_DIRS: Option<&str> = option_env!("RUSTOWL_RUNTIME_DIRS");
static CONFIG_RUNTIME_DIRS: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
    BUILD_RUNTIME_DIRS
        .map(|v| env::split_paths(v).collect())
        .unwrap_or_default()
});
const BUILD_SYSROOTS: Option<&str> = option_env!("RUSTOWL_SYSROOTS");
static CONFIG_SYSROOTS: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
    BUILD_SYSROOTS
        .map(|v| env::split_paths(v).collect())
        .unwrap_or_default()
});

const RUSTC_DRIVER_NAME: &str = env!("RUSTC_DRIVER_NAME");
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

pub fn rustc_driver_path(sysroot: impl AsRef<Path>) -> Option<PathBuf> {
    for file in recursive_read_dir(sysroot) {
        if file.file_name().unwrap().to_string_lossy() == RUSTC_DRIVER_NAME {
            log::info!("rustc_driver found: {}", file.display());
            return Some(file);
        }
    }
    None
}

pub fn sysroot_from_runtime(runtime: impl AsRef<Path>) -> PathBuf {
    runtime.as_ref().join("sysroot").join(TOOLCHAIN)
}

fn is_valid_sysroot(sysroot: impl AsRef<Path>) -> bool {
    rustc_driver_path(sysroot).is_some()
}

fn get_configured_runtime_dir() -> Option<PathBuf> {
    let env_var = env::var("RUSTOWL_RUNTIME_DIRS").unwrap_or_default();

    for runtime in env::split_paths(&env_var) {
        if is_valid_sysroot(sysroot_from_runtime(&runtime)) {
            log::info!("select runtime dir from env var: {}", runtime.display());
            return Some(runtime);
        }
    }

    for runtime in &*CONFIG_RUNTIME_DIRS {
        if is_valid_sysroot(sysroot_from_runtime(runtime)) {
            log::info!(
                "select runtime dir from build time env var: {}",
                runtime.display()
            );
            return Some(runtime.clone());
        }
    }
    None
}

pub fn check_fallback_dir() -> Option<PathBuf> {
    for fallback in &*FALLBACK_RUNTIME_DIRS {
        if is_valid_sysroot(sysroot_from_runtime(fallback)) {
            log::info!("select runtime from fallback: {}", fallback.display());
            return Some(fallback.clone());
        }
    }
    None
}

async fn get_runtime_dir() -> PathBuf {
    if let Some(runtime) = get_configured_runtime_dir() {
        return runtime;
    }
    if let Some(fallback) = check_fallback_dir() {
        return fallback;
    }

    log::info!("rustc_driver not found; start setup toolchain");
    let fallback = sysroot_from_runtime(&*FALLBACK_RUNTIME_DIRS[0]);
    if let Err(e) = setup_toolchain(&fallback).await {
        log::error!("{e:?}");
        std::process::exit(1);
    } else {
        fallback
    }
}

fn get_configured_sysroot() -> Option<PathBuf> {
    let env_var = env::var("RUSTOWL_SYSROOTS").unwrap_or_default();

    for sysroot in env::split_paths(&env_var) {
        if is_valid_sysroot(&sysroot) {
            log::info!("select sysroot dir from env var: {}", sysroot.display());
            return Some(sysroot);
        }
    }

    for sysroot in &*CONFIG_SYSROOTS {
        if is_valid_sysroot(sysroot) {
            log::info!(
                "select sysroot dir from build time env var: {}",
                sysroot.display(),
            );
            return Some(sysroot.clone());
        }
    }
    None
}

pub async fn get_sysroot() -> PathBuf {
    if let Some(sysroot) = get_configured_sysroot() {
        return sysroot;
    }

    // get sysroot from rustup
    if let Ok(child) = process::Command::new("rustup")
        .args(["run", TOOLCHAIN, "rustc", "--print=sysroot"])
        .stdout(Stdio::piped())
        .spawn()
        && let Ok(sysroot) = child
            .wait_with_output()
            .await
            .map(|v| PathBuf::from(String::from_utf8_lossy(&v.stdout).trim()))
        && is_valid_sysroot(&sysroot)
    {
        log::info!(
            "select sysroot dir from rustup installed: {}",
            sysroot.display(),
        );
        return sysroot;
    }

    // fallback sysroot
    sysroot_from_runtime(get_runtime_dir().await)
}

async fn download_tarball_and_extract(url: &str, dest: &Path) -> Result<(), ()> {
    log::info!("start downloading {url}...");
    let mut resp = match reqwest::get(url).await.and_then(|v| v.error_for_status()) {
        Ok(v) => v,
        Err(e) => {
            log::error!("failed to download tarball");
            log::error!("{e:?}");
            return Err(());
        }
    };

    let content_length = resp.content_length().unwrap_or(200_000_000) as usize;
    let mut data = Vec::with_capacity(content_length);
    let mut received = 0;
    while let Some(chunk) = match resp.chunk().await {
        Ok(v) => v,
        Err(e) => {
            log::error!("failed to download runtime archive");
            log::error!("{e:?}");
            return Err(());
        }
    } {
        data.extend_from_slice(&chunk);
        let current = data.len() * 100 / content_length;
        if received != current {
            received = current;
            log::info!("{received:>3}% received");
        }
    }
    log::info!("download finished");

    let decoder = GzDecoder::new(&*data);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest).map_err(|_| {
        log::error!("failed to unpack runtime tarball");
    })?;
    log::info!("successfully unpacked");
    Ok(())
}

async fn install_component(component: &str, dest: &Path) -> Result<(), ()> {
    let tempdir = tempfile::tempdir().map_err(|_| ())?;
    // Using `tempdir.path()` more than once causes SEGV, so we use `tempdir.path().to_owned()`.
    let temp_path = tempdir.path().to_owned();
    log::info!("temp dir is made: {}", temp_path.display());

    let dist_base = "https://static.rust-lang.org/dist";
    let base_url = match TOOLCHAIN_DATE {
        Some(v) => format!("{dist_base}/{v}"),
        None => dist_base.to_owned(),
    };

    let component_name = format!("{component}-{TOOLCHAIN_CHANNEL}-{HOST_TUPLE}");
    let tarball_url = format!("{base_url}/{component_name}.tar.gz");

    download_tarball_and_extract(&tarball_url, &temp_path).await?;

    let extracted_path = temp_path.join(component_name);
    let components = read_to_string(extracted_path.join("components"))
        .await
        .map_err(|_| {
            log::error!("failed to read components list");
        })?;
    let components = components.split_whitespace();

    for component in components {
        let component_path = extracted_path.join(component);
        for from in recursive_read_dir(&component_path) {
            let rel_path = match from.strip_prefix(&component_path) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("path error: {e}");
                    return Err(());
                }
            };
            let to = dest.join(rel_path);
            if let Err(e) = create_dir_all(to.parent().unwrap()).await {
                log::error!("failed to create dir: {e}");
                return Err(());
            }
            if let Err(e) = rename(&from, &to).await {
                log::warn!("file rename failed: {e}, falling back to copy and delete");
                if let Err(copy_err) = tokio::fs::copy(&from, &to).await {
                    log::error!("file copy error (after rename failure): {copy_err}");
                    return Err(());
                }
                if let Err(del_err) = tokio::fs::remove_file(&from).await {
                    log::error!("file delete error (after copy): {del_err}");
                    return Err(());
                }
            }
        }
        log::info!("component {component} successfully installed");
    }
    Ok(())
}
pub async fn setup_toolchain(dest: impl AsRef<Path>) -> Result<(), ()> {
    if create_dir_all(&dest.as_ref()).await.is_err() {
        log::error!("failed to create toolchain directory");
        return Err(());
    }

    install_component("rustc", dest.as_ref()).await?;
    install_component("rust-std", dest.as_ref()).await?;
    install_component("cargo", dest.as_ref()).await?;

    log::info!("toolchain setup finished");
    Ok(())
}

pub async fn uninstall_toolchain() {
    for fallback in &*FALLBACK_RUNTIME_DIRS {
        let sysroot = sysroot_from_runtime(fallback);
        if sysroot.is_dir() {
            log::info!("remove sysroot: {}", sysroot.display());
            remove_dir_all(&sysroot).await.unwrap();
        }
    }
}

pub async fn get_executable_path(name: &str) -> String {
    #[cfg(not(windows))]
    let exec_name = name.to_owned();
    #[cfg(windows)]
    let exec_name = format!("{name}.exe");

    let runtime_dir = get_runtime_dir().await;
    let exec = runtime_dir.join(&exec_name);
    if exec.is_file() {
        log::info!("{name} is selected in runtime root");
        return exec.to_string_lossy().to_string();
    }

    let sysroot = sysroot_from_runtime(runtime_dir);
    let exec_bin = sysroot.join("bin").join(&exec_name);
    if exec_bin.is_file() {
        log::info!("{name} is selected in sysroot/bin");
        return exec_bin.to_string_lossy().to_string();
    }

    let mut current_exec = env::current_exe().unwrap();
    current_exec.set_file_name(&exec_name);
    if current_exec.is_file() {
        log::info!("{name} is selected in the same directory as rustowl executable");
        return current_exec.to_string_lossy().to_string();
    }

    log::warn!("{name} not found; fallback");
    exec_name.to_owned()
}

pub fn set_rustc_env(command: &mut tokio::process::Command, sysroot: &Path) {
    command
        .env("RUSTC_BOOTSTRAP", "1") // Support nightly projects
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            format!("--sysroot={}", sysroot.display()),
        );

    let driver_dir = match rustc_driver_path(sysroot) {
        Some(v) => v,
        None => {
            log::warn!("unable to find rustc_driver");
            return;
        }
    }
    .parent()
    .unwrap()
    .to_path_buf();

    #[cfg(target_os = "linux")]
    {
        let mut paths = env::split_paths(&env::var("LD_LIBRARY_PATH").unwrap_or("".to_owned()))
            .collect::<std::collections::VecDeque<_>>();
        paths.push_front(sysroot.join(driver_dir));
        let paths = env::join_paths(paths).unwrap();
        command.env("LD_LIBRARY_PATH", paths);
    }
    #[cfg(target_os = "macos")]
    {
        let mut paths =
            env::split_paths(&env::var("DYLD_FALLBACK_LIBRARY_PATH").unwrap_or("".to_owned()))
                .collect::<std::collections::VecDeque<_>>();
        paths.push_front(sysroot.join(driver_dir));
        let paths = env::join_paths(paths).unwrap();
        command.env("DYLD_FALLBACK_LIBRARY_PATH", paths);
    }
    #[cfg(target_os = "windows")]
    {
        let mut paths = env::split_paths(&env::var_os("Path").unwrap())
            .collect::<std::collections::VecDeque<_>>();
        paths.push_front(sysroot.join(driver_dir));
        let paths = env::join_paths(paths).unwrap();
        command.env("Path", paths);
    }

    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}
