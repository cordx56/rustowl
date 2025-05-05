use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::LazyLock;
use tokio::{
    fs::{create_dir_all, remove_dir_all},
    io::AsyncReadExt,
    process::Command,
    sync::OnceCell,
};

#[cfg(not(windows))]
use flate2::read::GzDecoder;
#[cfg(not(windows))]
use tar::Archive;
#[cfg(windows)]
use zip::ZipArchive;

pub const TOOLCHAIN: &str = env!("RUSTOWL_TOOLCHAIN");
const BUILD_RUNTIME_DIRS: Option<&str> = option_env!("RUSTOWL_RUNTIME_DIRS");

static FALLBACK_RUNTIME: LazyLock<PathBuf> = LazyLock::new(|| {
    env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("rustowl-runtime")
});

static CONFIG_RUNTIME_DIRS: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
    BUILD_RUNTIME_DIRS
        .map(|v| env::split_paths(v).collect())
        .unwrap_or_default()
});

static RUSTUP_SYSROOT: OnceCell<Option<PathBuf>> = OnceCell::const_new();

const ARCHIVE_NAME: &str = env!("RUSTOWL_ARCHIVE_NAME");

pub fn get_configured_runtime_dir() -> Option<PathBuf> {
    let env_var = env::var("RUSTOWL_RUNTIME_DIRS").unwrap_or_default();

    for runtime in env::split_paths(&env_var) {
        if runtime.is_dir() {
            log::info!("select runtime dir from env var: {}", runtime.display());
            return Some(runtime);
        }
    }

    for runtime in &*CONFIG_RUNTIME_DIRS {
        if runtime.is_dir() {
            log::info!(
                "select runtime dir from build time env var: {}",
                runtime.display()
            );
            return Some(runtime.clone());
        }
    }
    None
}
pub async fn get_runtime_dir() -> PathBuf {
    if let Some(runtime) = get_configured_runtime_dir() {
        runtime
    } else if !FALLBACK_RUNTIME.is_dir() && setup_toolchain().await.is_err() {
        std::process::exit(1);
    } else {
        FALLBACK_RUNTIME.clone()
    }
}
pub async fn get_sysroot() -> PathBuf {
    if let Some(runtime) = get_configured_runtime_dir() {
        let sysroot = runtime.join("sysroot").join(TOOLCHAIN);
        if sysroot.is_dir() {
            log::info!(
                "select sysroot from configured runtime dir: {}",
                sysroot.display()
            );
            return sysroot;
        }
    }
    if let Some(sysroot) = RUSTUP_SYSROOT
        .get_or_init(|| async {
            if let Ok(mut child) = Command::new("rustup")
                .args(["run", TOOLCHAIN, "rustc", "--print=sysroot"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                let mut stdout = child.stdout.take().unwrap();
                if let Ok(status) = child.wait().await {
                    if status.success() {
                        let mut output = String::new();
                        if stdout.read_to_string(&mut output).await.is_ok() {
                            return Some(PathBuf::from(output.trim()));
                        }
                    }
                }
            }
            None
        })
        .await
    {
        log::info!("select sysroot from rustup: {}", sysroot.display());
        return sysroot.to_owned();
    }
    let sysroot = FALLBACK_RUNTIME.join("sysroot").join(TOOLCHAIN);
    if !sysroot.is_dir() {
        log::info!("sysroot not found; start setup toolchain");
        if setup_toolchain().await.is_err() {
            std::process::exit(1);
        }
    }
    sysroot
}

pub async fn setup_toolchain() -> Result<PathBuf, ()> {
    log::info!("start downloading {ARCHIVE_NAME}...");
    let tarball_url = format!(
        "https://github.com/cordx56/rustowl/releases/download/v{}/{ARCHIVE_NAME}",
        clap::crate_version!(),
    );

    let resp = match reqwest::get(&tarball_url)
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

    let bytes = match resp.bytes().await {
        Ok(v) => v,
        Err(e) => {
            log::error!("failed to download runtime archive");
            log::error!("{e:?}");
            return Err(());
        }
    };
    log::info!("download finished");

    if create_dir_all(&*FALLBACK_RUNTIME).await.is_err() {
        log::error!("failed to create toolchain directory");
        return Err(());
    }

    #[cfg(windows)]
    {
        let cursor = std::io::Cursor::new(&*bytes);
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
                Some(path) => FALLBACK_RUNTIME.join(path),
                None => continue,
            };

            if file.name().ends_with('/') {
                if let Err(e) = std::fs::create_dir_all(&outpath) {
                    log::error!("failed to create directory {}", outpath.display());
                    log::error!("{e:?}");
                    continue;
                }
            } else {
                if let Some(parent) = outpath.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        log::error!("failed to create parent directory {}", parent.display());
                        log::error!("{e:?}");
                        continue;
                    }
                }
                let mut outfile = match std::fs::File::create(&outpath) {
                    Ok(file) => file,
                    Err(e) => {
                        log::error!("failed to create file {}", outpath.display());
                        log::error!("{e:?}");
                        continue;
                    }
                };
                if let Err(e) = std::io::copy(&mut file, &mut outfile) {
                    log::error!("failed to write file {}", outpath.display());
                    log::error!("{e:?}");
                    continue;
                }
            }
            log::info!("{} unpacked", outpath.display());
        }
    }

    #[cfg(not(windows))]
    {
        let decoder = GzDecoder::new(&*bytes);
        let mut archive = Archive::new(decoder);
        if let Ok(entries) = archive.entries() {
            for mut entry in entries.flatten() {
                if let Ok(path) = entry.path() {
                    let path = path.to_path_buf();
                    if path.as_os_str() != "rustowl" {
                        if !entry.unpack_in(&*FALLBACK_RUNTIME).unwrap_or(false) {
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

    log::info!("runtime setup done in {}", FALLBACK_RUNTIME.display());
    Ok(FALLBACK_RUNTIME.clone())
}

pub async fn uninstall_toolchain() {
    remove_dir_all(&*FALLBACK_RUNTIME).await.unwrap();
}
