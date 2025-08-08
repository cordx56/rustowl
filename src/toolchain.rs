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

pub static FALLBACK_RUNTIME_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    env::home_dir()
        .map(|v| v.join(".rustowl"))
        .unwrap_or(PathBuf::from("/opt/rustowl"))
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

    log::info!("sysroot not found; start setup toolchain");
    if let Err(e) = setup_toolchain(&*FALLBACK_RUNTIME_DIR, false).await {
        log::error!("{e:?}");
        std::process::exit(1);
    } else {
        FALLBACK_RUNTIME_DIR.clone()
    }
}

pub async fn get_sysroot() -> PathBuf {
    sysroot_from_runtime(get_runtime_dir().await)
}

async fn download(url: &str) -> Result<Vec<u8>, ()> {
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
    Ok(data)
}
async fn download_tarball_and_extract(url: &str, dest: &Path) -> Result<(), ()> {
    let data = download(url).await?;
    let decoder = GzDecoder::new(&*data);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest).map_err(|_| {
        log::error!("failed to unpack tarball");
    })?;
    log::info!("successfully unpacked");
    Ok(())
}
#[cfg(target_os = "windows")]
async fn download_zip_and_extract(url: &str, dest: &Path) -> Result<(), ()> {
    use zip::ZipArchive;
    let data = download(url).await?;
    let cursor = std::io::Cursor::new(&*data);

    let mut archive = match ZipArchive::new(cursor) {
        Ok(archive) => archive,
        Err(e) => {
            log::error!("failed to read ZIP archive");
            log::error!("{e:?}");
            return Err(());
        }
    };
    archive.extract(dest).map_err(|e| {
        log::error!("failed to unpack zip: {e}");
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

    let component_toolchain = format!("{component}-{TOOLCHAIN_CHANNEL}-{HOST_TUPLE}");
    let tarball_url = format!("{base_url}/{component_toolchain}.tar.gz");

    download_tarball_and_extract(&tarball_url, &temp_path).await?;

    let extracted_path = temp_path.join(&component_toolchain);
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
pub async fn setup_toolchain(dest: impl AsRef<Path>, skip_rustowl: bool) -> Result<(), ()> {
    setup_rust_toolchain(&dest).await?;
    if !skip_rustowl {
        setup_rustowl_toolchain(&dest).await?;
    }
    Ok(())
}
pub async fn setup_rust_toolchain(dest: impl AsRef<Path>) -> Result<(), ()> {
    let sysroot = sysroot_from_runtime(dest.as_ref());
    if create_dir_all(&sysroot).await.is_err() {
        log::error!("failed to create toolchain directory");
        return Err(());
    }

    log::info!("start installing Rust toolchain...");
    install_component("rustc", &sysroot).await?;
    install_component("rust-std", &sysroot).await?;
    install_component("cargo", &sysroot).await?;
    log::info!("installing Rust toolchain finished");
    Ok(())
}
pub async fn setup_rustowl_toolchain(dest: impl AsRef<Path>) -> Result<(), ()> {
    log::info!("start installing RustOwl toolchain...");
    #[cfg(not(target_os = "windows"))]
    let rustowl_toolchain_result = {
        let rustowl_tarball_url = format!(
            "https://github.com/cordx56/rustowl/releases/download/v{}/rustowl-{HOST_TUPLE}.tar.gz",
            clap::crate_version!(),
        );
        download_tarball_and_extract(&rustowl_tarball_url, dest.as_ref()).await
    };
    #[cfg(target_os = "windows")]
    let rustowl_toolchain_result = {
        let rustowl_zip_url = format!(
            "https://github.com/cordx56/rustowl/releases/download/v{}/rustowl-{HOST_TUPLE}.zip",
            clap::crate_version!(),
        );
        download_zip_and_extract(&rustowl_zip_url, dest.as_ref()).await
    };
    if rustowl_toolchain_result.is_ok() {
        log::info!("installing RustOwl toolchain finished");
    } else {
        log::warn!("could not install RustOwl toolchain; local installed rustowlc will be used");
    }

    log::info!("toolchain setup finished");
    Ok(())
}

pub async fn uninstall_toolchain() {
    let sysroot = sysroot_from_runtime(&*FALLBACK_RUNTIME_DIR);
    if sysroot.is_dir() {
        log::info!("remove sysroot: {}", sysroot.display());
        remove_dir_all(&sysroot).await.unwrap();
    }
}

pub async fn get_executable_path(name: &str) -> String {
    #[cfg(not(windows))]
    let exec_name = name.to_owned();
    #[cfg(windows)]
    let exec_name = format!("{name}.exe");

    let sysroot = get_sysroot().await;
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

    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}
