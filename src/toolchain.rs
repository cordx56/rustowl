use std::env;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use tokio::{
    fs::{create_dir_all, remove_dir_all},
    process,
};

#[cfg(not(windows))]
use flate2::read::GzDecoder;
#[cfg(not(windows))]
use tar::Archive;
#[cfg(windows)]
use zip::ZipArchive;

pub const TOOLCHAIN: &str = env!("RUSTOWL_TOOLCHAIN");

static FALLBACK_RUNTIME_DIRS: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
    let exec_dir = env::current_exe().unwrap().parent().unwrap().to_path_buf();
    vec![exec_dir.join("rustowl-runtime"), exec_dir]
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

const ARCHIVE_NAME: &str = env!("RUSTOWL_ARCHIVE_NAME");

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

fn sysroot_from_runtime(runtime: impl AsRef<Path>) -> PathBuf {
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
    match setup_toolchain().await {
        Ok(v) => v,
        Err(e) => {
            log::error!("{e:?}");
            std::process::exit(1);
        }
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
            && is_valid_sysroot(&sysroot) {
                log::info!(
                    "select sysroot dir from rustup installed: {}",
                    sysroot.display(),
                );
                return sysroot;
            }

    // fallback sysroot
    sysroot_from_runtime(get_runtime_dir().await)
}

pub async fn setup_toolchain() -> Result<PathBuf, ()> {
    log::info!("start downloading {ARCHIVE_NAME}...");
    let tarball_url = format!(
        "https://github.com/cordx56/rustowl/releases/download/v{}/{ARCHIVE_NAME}",
        clap::crate_version!(),
    );

    let mut resp = match reqwest::get(&tarball_url)
        .await
        .and_then(|v| v.error_for_status())
    {
        Ok(v) => v,
        Err(e) => {
            log::error!("failed to download runtime archive");
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

    let fallback = FALLBACK_RUNTIME_DIRS[0].clone();
    if create_dir_all(&fallback).await.is_err() {
        log::error!("failed to create toolchain directory");
        return Err(());
    }

    #[cfg(windows)]
    {
        let cursor = std::io::Cursor::new(&*data);
        let mut archive = match ZipArchive::new(cursor) {
            Ok(archive) => archive,
            Err(e) => {
                log::error!("failed to read ZIP archive");
                log::error!("{e:?}");
                return Err(());
            }
        };

        for i in 0..archive.len() {
            let mut file = match archive.by_index(i) {
                Ok(file) => file,
                Err(e) => {
                    log::error!("failed to read ZIP entry");
                    log::error!("{e:?}");
                    continue;
                }
            };

            let outpath = match file.enclosed_name() {
                Some(path) => fallback.join(path),
                None => continue,
            };

            if file.is_dir() {
                log::info!("File {} extracted to \"{}\"", i, outpath.display());
                std::fs::create_dir_all(&outpath).unwrap();
            } else {
                log::info!(
                    "File {} extracted to \"{}\" ({} bytes)",
                    i,
                    outpath.display(),
                    file.size()
                );
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p).unwrap();
                    }
                }
                let mut outfile = std::fs::File::create(&outpath).unwrap();
                std::io::copy(&mut file, &mut outfile).unwrap();
            }

            log::info!("{} unpacked", outpath.display());
        }
    }

    #[cfg(not(windows))]
    {
        let decoder = GzDecoder::new(&*data);
        let mut archive = Archive::new(decoder);
        if let Ok(entries) = archive.entries() {
            for mut entry in entries.flatten() {
                if let Ok(path) = entry.path() {
                    let path = path.to_path_buf();
                    if path.as_os_str() != "rustowl" {
                        if !entry.unpack_in(&fallback).unwrap_or(false) {
                            log::error!("failed to unpack runtime tarball");
                            return Err(());
                        }
                        log::info!("{} unpacked", path.display());
                    }
                }
            }
        } else {
            log::error!("failed to unpack runtime tarball");
            return Err(());
        }
    }

    log::info!("runtime setup done in {}", fallback.display());
    Ok(fallback)
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

pub async fn get_rustowlc_path() -> String {
    let mut current_rustowlc = env::current_exe().unwrap();
    #[cfg(not(windows))]
    current_rustowlc.set_file_name("rustowlc");
    #[cfg(windows)]
    current_rustowlc.set_file_name("rustowlc.exe");
    if current_rustowlc.is_file() {
        log::info!("rustowlc is selected in the same directory as rustowl executable");
        return current_rustowlc.to_string_lossy().to_string();
    }

    if process::Command::new("rustowlc").spawn().is_ok() {
        log::info!("rustowlc is selected in PATH");
        return "rustowlc".to_owned();
    }

    let runtime_dir = get_runtime_dir().await;
    #[cfg(not(windows))]
    let rustowlc = runtime_dir.join("rustowlc");
    #[cfg(windows)]
    let rustowlc = runtime_dir.join("rustowlc.exe");
    if rustowlc.is_file() {
        rustowlc.to_string_lossy().to_string()
    } else {
        log::warn!("rustowlc not found; fallback");
        "rustowlc".to_owned()
    }
}

pub fn set_rustc_env(command: &mut tokio::process::Command, sysroot: &Path) {
    command
        .env("RUSTC_BOOTSTRAP", "1") // Support nightly projects
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            format!("--sysroot={}", sysroot.display()),
        );

    let driver_dir = rustc_driver_path(sysroot)
        .unwrap()
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
