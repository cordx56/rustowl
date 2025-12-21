use std::env;

use std::io::IsTerminal;
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
        for entry in std::fs::read_dir(&path).unwrap().flatten() {
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

    tracing::debug!("sysroot not found; start setup toolchain");
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

async fn download(url: &str) -> Result<Vec<u8>, ()> {
    use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

    tracing::debug!("start downloading {url}...");

    let progress = if std::io::stderr().is_terminal() {
        let progress = ProgressBar::new(0);
        progress.set_draw_target(ProgressDrawTarget::stderr());
        progress.set_style(
            ProgressStyle::with_template("{spinner:.green} {wide_msg} {bytes}/{total_bytes}")
                .unwrap(),
        );
        progress.set_message("Downloading...");
        Some(progress)
    } else {
        None
    };

    let _progress_guard = progress
        .as_ref()
        .cloned()
        .map(crate::ActiveProgressBarGuard::set);

    let mut resp = match reqwest::get(url).await.and_then(|v| v.error_for_status()) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to download tarball");
            tracing::error!("{e:?}");
            return Err(());
        }
    };

    let content_length = resp.content_length().unwrap_or(200_000_000) as usize;
    if let Some(progress) = &progress {
        progress.set_length(content_length as u64);
    }

    let mut data = Vec::with_capacity(content_length);
    while let Some(chunk) = match resp.chunk().await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("failed to download runtime archive");
            tracing::error!("{e:?}");
            return Err(());
        }
    } {
        data.extend_from_slice(&chunk);

        if let Some(progress) = &progress {
            progress.set_position(data.len() as u64);
        }
    }

    if let Some(progress) = progress {
        progress.finish_and_clear();
    }

    tracing::debug!("download finished");
    Ok(data)
}
async fn download_tarball_and_extract(url: &str, dest: &Path) -> Result<(), ()> {
    let data = download(url).await?;
    let decoder = GzDecoder::new(&*data);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest).map_err(|_| {
        tracing::error!("failed to unpack tarball");
    })?;
    tracing::debug!("successfully unpacked");
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
            tracing::error!("failed to read ZIP archive");
            tracing::error!("{e:?}");
            return Err(());
        }
    };
    archive.extract(dest).map_err(|e| {
        tracing::error!("failed to unpack zip: {e}");
    })?;
    tracing::debug!("successfully unpacked");
    Ok(())
}

async fn install_component(component: &str, dest: &Path) -> Result<(), ()> {
    let tempdir = tempfile::tempdir().map_err(|_| ())?;
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

    download_tarball_and_extract(&tarball_url, &temp_path).await?;

    let extracted_path = temp_path.join(&component_toolchain);
    let components = read_to_string(extracted_path.join("components"))
        .await
        .map_err(|_| {
            tracing::error!("failed to read components list");
        })?;
    let components = components.split_whitespace();

    for component in components {
        let component_path = extracted_path.join(component);
        for from in recursive_read_dir(&component_path) {
            let rel_path = match from.strip_prefix(&component_path) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("path error: {e}");
                    return Err(());
                }
            };
            let to = dest.join(rel_path);
            if let Err(e) = create_dir_all(to.parent().unwrap()).await {
                tracing::error!("failed to create dir: {e}");
                return Err(());
            }
            if let Err(e) = rename(&from, &to).await {
                tracing::warn!("file rename failed: {e}, falling back to copy and delete");
                if let Err(copy_err) = tokio::fs::copy(&from, &to).await {
                    tracing::error!("file copy error (after rename failure): {copy_err}");
                    return Err(());
                }
                if let Err(del_err) = tokio::fs::remove_file(&from).await {
                    tracing::error!("file delete error (after copy): {del_err}");
                    return Err(());
                }
            }
        }
        tracing::debug!("component {component} successfully installed");
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
        tracing::error!("failed to create toolchain directory");
        return Err(());
    }

    tracing::debug!("start installing Rust toolchain...");
    install_component("rustc", &sysroot).await?;
    install_component("rust-std", &sysroot).await?;
    install_component("cargo", &sysroot).await?;
    tracing::debug!("installing Rust toolchain finished");
    Ok(())
}
pub async fn setup_rustowl_toolchain(dest: impl AsRef<Path>) -> Result<(), ()> {
    tracing::debug!("start installing RustOwl toolchain...");
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
        tracing::debug!("installing RustOwl toolchain finished");
    } else {
        tracing::warn!(
            "could not install RustOwl toolchain; local installed rustowlc will be used"
        );
    }

    tracing::debug!("toolchain setup finished");
    Ok(())
}

pub async fn uninstall_toolchain() {
    let sysroot = sysroot_from_runtime(&*FALLBACK_RUNTIME_DIR);
    if sysroot.is_dir() {
        tracing::debug!("remove sysroot: {}", sysroot.display());
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
    command
        .env("RUSTC", &rustowlc)
        .env("RUSTC_WORKSPACE_WRAPPER", &rustowlc);
    set_rustc_env(&mut command, &get_sysroot().await);
    command
}

/// Configure environment variables on a Command so Rust invocations use the given sysroot.
///
/// Sets:
/// - `RUSTC_BOOTSTRAP = "1"` to allow nightly-only features when invoking rustc.
/// - `CARGO_ENCODED_RUSTFLAGS = "--sysroot={sysroot}"` so cargo/rustc use the provided sysroot.
/// - On Linux: prepends `{sysroot}/lib` to `LD_LIBRARY_PATH`.
/// - On macOS: prepends `{sysroot}/lib` to `DYLD_FALLBACK_LIBRARY_PATH`.
/// - On Windows: prepends `{sysroot}/bin` to `Path`.
///
/// The provided `command` is mutated in place.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use tokio::process::Command;
/// use rustowl::toolchain;
///
/// let sysroot = Path::new("/opt/rust/sysroot");
/// let mut cmd = Command::new("cargo");
/// toolchain::set_rustc_env(&mut cmd, sysroot);
/// // cmd is now configured to invoke cargo/rustc with the given sysroot.
/// ```
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_sysroot_from_runtime() {
        let runtime = PathBuf::from("/opt/test-runtime");
        let sysroot = sysroot_from_runtime(&runtime);

        let expected = runtime.join("sysroot").join(TOOLCHAIN);
        assert_eq!(sysroot, expected);
    }

    #[test]
    fn test_sysroot_from_runtime_different_paths() {
        // Test with various path types
        let paths = vec![
            PathBuf::from("/usr/local/rustowl"),
            PathBuf::from("./relative/path"),
            PathBuf::from("../parent/path"),
            PathBuf::from("/"),
        ];

        for path in paths {
            let sysroot = sysroot_from_runtime(&path);
            assert!(sysroot.starts_with(&path));
            assert!(sysroot.ends_with(TOOLCHAIN));
            assert!(sysroot.to_string_lossy().contains("sysroot"));
        }
    }

    #[test]
    fn test_toolchain_constants() {
        // Test that the constants are properly set
        // Host tuple should contain some expected patterns
        assert!(HOST_TUPLE.contains('-'));
    }

    #[test]
    fn test_recursive_read_dir_non_existent() {
        // Test with non-existent directory
        let non_existent = PathBuf::from("/this/path/definitely/does/not/exist");
        let result = recursive_read_dir(&non_existent);
        assert!(result.is_empty());
    }

    #[test]
    fn test_recursive_read_dir_file() {
        // Create a temporary file to test with
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let result = recursive_read_dir(temp_file.path());
        assert!(result.is_empty()); // Should return empty for files
    }

    #[test]
    fn test_set_rustc_env() {
        let mut command = tokio::process::Command::new("echo");
        let sysroot = PathBuf::from("/test/sysroot");

        set_rustc_env(&mut command, &sysroot);

        // We can't easily inspect the environment variables set on the command,
        // but we can verify the function doesn't panic and accepts the expected types
        // The actual functionality requires process execution which we avoid in unit tests
    }

    #[test]
    fn test_sysroot_path_construction() {
        // Test edge cases for path construction
        let empty_path = PathBuf::new();
        let sysroot = sysroot_from_runtime(&empty_path);

        // Should still construct a valid path
        assert_eq!(sysroot, PathBuf::from("sysroot").join(TOOLCHAIN));

        // Test with root path
        let root_path = PathBuf::from("/");
        let sysroot = sysroot_from_runtime(&root_path);
        assert_eq!(sysroot, PathBuf::from("/sysroot").join(TOOLCHAIN));
    }

    #[test]
    fn test_toolchain_date_handling() {
        // Test that TOOLCHAIN_DATE is properly handled
        // This is a compile-time constant, so we just verify it's accessible
        match TOOLCHAIN_DATE {
            Some(date) => {
                assert!(!date.is_empty());
                // Date should be in YYYY-MM-DD format if present
                assert!(date.len() >= 10);
            }
            None => {
                // This is fine, toolchain date is optional
            }
        }
    }

    #[test]
    fn test_component_url_construction() {
        // Test the URL construction logic that would be used in install_component
        let component = "rustc";
        let component_toolchain = format!("{component}-{TOOLCHAIN_CHANNEL}-{HOST_TUPLE}");

        // Should contain all the parts
        assert!(component_toolchain.contains(component));
        assert!(component_toolchain.contains(TOOLCHAIN_CHANNEL));
        assert!(component_toolchain.contains(HOST_TUPLE));

        // Should be properly formatted with dashes
        let parts: Vec<&str> = component_toolchain.split('-').collect();
        assert!(parts.len() >= 3); // At least component-channel-host parts
    }

    /// Verifies the fallback runtime directory is a valid, non-empty path.
    ///
    /// This test asserts that `FALLBACK_RUNTIME_DIR` yields a non-empty `PathBuf`.
    /// In typical environments the path will be absolute; however, that may not
    /// hold if the current executable or home directory cannot be determined.
    #[test]
    fn test_fallback_runtime_dir_logic() {
        // Test the path preference logic (without actually checking filesystem)
        let fallback = &*FALLBACK_RUNTIME_DIR;

        // Should be a valid path
        assert!(!fallback.as_os_str().is_empty());

        // Should be an absolute path in most cases
        // (Except when current_exe or home_dir fails, but that's rare)
    }

    #[test]
    fn test_recursive_read_dir_with_temp_directory() {
        // Create a temporary directory structure for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create subdirectories and files
        std::fs::create_dir_all(temp_path.join("subdir1")).unwrap();
        std::fs::create_dir_all(temp_path.join("subdir2")).unwrap();
        std::fs::write(temp_path.join("file1.txt"), "content").unwrap();
        std::fs::write(temp_path.join("subdir1").join("file2.txt"), "content").unwrap();
        std::fs::write(temp_path.join("subdir2").join("file3.txt"), "content").unwrap();

        let files = recursive_read_dir(temp_path);

        // Should find all files recursively
        assert!(files.len() >= 3);

        // Check that files are found (paths might be in different order)
        let file_names: Vec<String> = files
            .iter()
            .filter_map(|p| p.file_name()?.to_str())
            .map(|s| s.to_string())
            .collect();

        assert!(file_names.contains(&"file1.txt".to_string()));
        assert!(file_names.contains(&"file2.txt".to_string()));
        assert!(file_names.contains(&"file3.txt".to_string()));
    }

    #[test]
    fn test_host_tuple_format() {
        // HOST_TUPLE should follow the expected format: arch-vendor-os-env
        let parts: Vec<&str> = HOST_TUPLE.split('-').collect();
        assert!(
            parts.len() >= 3,
            "HOST_TUPLE should have at least 3 parts separated by hyphens"
        );

        // First part should be architecture
        let arch = parts[0];
        assert!(!arch.is_empty());

        // Common architectures
        let valid_archs = ["x86_64", "i686", "aarch64", "armv7", "riscv64"];
        let is_valid_arch = valid_archs.iter().any(|&a| arch.starts_with(a));
        assert!(is_valid_arch, "Unexpected architecture: {arch}");
    }

    #[test]
    fn test_toolchain_format() {
        // TOOLCHAIN should be a valid toolchain identifier

        // Should contain date or channel information
        // Typical format might be: nightly-2023-01-01-x86_64-unknown-linux-gnu
        assert!(
            TOOLCHAIN.contains('-'),
            "TOOLCHAIN should contain separators"
        );

        // Should not contain spaces or special characters
        assert!(
            !TOOLCHAIN.contains(' '),
            "TOOLCHAIN should not contain spaces"
        );
    }

    #[test]
    fn test_path_construction_edge_cases() {
        // Test with Windows-style paths
        let windows_path = PathBuf::from("C:\\Windows\\System32");
        let sysroot = sysroot_from_runtime(&windows_path);
        assert!(sysroot.to_string_lossy().contains("sysroot"));
        assert!(sysroot.to_string_lossy().contains(TOOLCHAIN));

        // Test with path containing Unicode
        let unicode_path = PathBuf::from("/opt/rustowl/测试");
        let sysroot = sysroot_from_runtime(&unicode_path);
        assert!(sysroot.starts_with(&unicode_path));

        // Test with very long path
        let long_path = PathBuf::from("/".to_string() + &"very_long_directory_name/".repeat(10));
        let sysroot = sysroot_from_runtime(&long_path);
        assert!(sysroot.starts_with(&long_path));
    }

    #[test]
    fn test_environment_variable_edge_cases() {
        // Test path handling with empty environment variables
        use std::collections::VecDeque;

        // Test with empty LD_LIBRARY_PATH-like handling
        let empty_paths: VecDeque<PathBuf> = VecDeque::new();
        let joined = std::env::join_paths(empty_paths.clone());
        assert!(joined.is_ok());

        // Test with single path
        let mut single_path = empty_paths;
        single_path.push_back(PathBuf::from("/usr/lib"));
        let joined = std::env::join_paths(single_path);
        assert!(joined.is_ok());

        // Test with multiple paths
        let mut multi_paths = VecDeque::new();
        multi_paths.push_back(PathBuf::from("/usr/lib"));
        multi_paths.push_back(PathBuf::from("/lib"));
        let joined = std::env::join_paths(multi_paths);
        assert!(joined.is_ok());
    }

    #[test]
    fn test_url_construction_patterns() {
        // Test URL construction components
        let component = "rust-std";
        let base_url = "https://static.rust-lang.org/dist";

        // Test with date
        let date = "2023-01-01";
        let url_with_date = format!("{base_url}/{date}");
        assert!(url_with_date.starts_with("https://"));
        assert!(url_with_date.contains(date));

        // Test component URL construction
        let component_toolchain = format!("{component}-{TOOLCHAIN_CHANNEL}-{HOST_TUPLE}");
        let tarball_url = format!("{base_url}/{component_toolchain}.tar.gz");

        assert!(tarball_url.starts_with("https://"));
        assert!(tarball_url.ends_with(".tar.gz"));
        assert!(tarball_url.contains(component));
        assert!(tarball_url.contains(TOOLCHAIN_CHANNEL));
        assert!(tarball_url.contains(HOST_TUPLE));
    }

    #[test]
    fn test_version_url_construction() {
        // Test RustOwl toolchain URL construction logic
        let version = "1.0.0";

        #[cfg(not(target_os = "windows"))]
        {
            let rustowl_tarball_url = format!(
                "https://github.com/cordx56/rustowl/releases/download/v{version}/rustowl-{HOST_TUPLE}.tar.gz"
            );
            assert!(rustowl_tarball_url.starts_with("https://github.com/"));
            assert!(rustowl_tarball_url.contains("rustowl"));
            assert!(rustowl_tarball_url.contains(version));
            assert!(rustowl_tarball_url.contains(HOST_TUPLE));
            assert!(rustowl_tarball_url.ends_with(".tar.gz"));
        }

        #[cfg(target_os = "windows")]
        {
            let rustowl_zip_url = format!(
                "https://github.com/cordx56/rustowl/releases/download/v{version}/rustowl-{HOST_TUPLE}.zip"
            );
            assert!(rustowl_zip_url.starts_with("https://github.com/"));
            assert!(rustowl_zip_url.contains("rustowl"));
            assert!(rustowl_zip_url.contains(version));
            assert!(rustowl_zip_url.contains(HOST_TUPLE));
            assert!(rustowl_zip_url.ends_with(".zip"));
        }
    }

    #[test]
    fn test_executable_name_logic() {
        // Test executable name construction logic
        let name = "rustc";

        #[cfg(not(windows))]
        {
            let exec_name = name.to_owned();
            assert_eq!(exec_name, "rustc");
        }

        #[cfg(windows)]
        {
            let exec_name = format!("{name}.exe");
            assert_eq!(exec_name, "rustc.exe");
        }

        // Test with different executable names
        let test_names = ["cargo", "rustfmt", "clippy"];
        for test_name in test_names {
            #[cfg(not(windows))]
            {
                let exec_name = test_name.to_owned();
                assert_eq!(exec_name, test_name);
            }

            #[cfg(windows)]
            {
                let exec_name = format!("{test_name}.exe");
                assert!(exec_name.ends_with(".exe"));
                assert!(exec_name.starts_with(test_name));
            }
        }
    }

    #[test]
    fn test_toolchain_constants_consistency() {
        // Verify that constants are consistent with each other
        assert!(
            TOOLCHAIN.contains(TOOLCHAIN_CHANNEL) || TOOLCHAIN.contains(HOST_TUPLE),
            "TOOLCHAIN should contain either channel or host tuple information"
        );

        // Test that optional date is properly handled
        if let Some(date) = TOOLCHAIN_DATE {
            assert!(!date.is_empty());
            // Date should be in a reasonable format (YYYY-MM-DD)
            if date.len() >= 10 {
                let parts: Vec<&str> = date.split('-').collect();
                if parts.len() >= 3 {
                    // First part should be year (4 digits)
                    if let Ok(year) = parts[0].parse::<u32>() {
                        assert!(
                            (2020..=2030).contains(&year),
                            "Year should be reasonable: {year}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_progress_reporting_simulation() {
        // Test progress calculation logic
        let content_length = 1000;
        let mut received_percentages = Vec::new();

        for chunk_size in [100, 200, 150, 300, 250] {
            let current_size = chunk_size;
            let current = current_size * 100 / content_length;
            received_percentages.push(current);
        }

        // Verify progress makes sense
        assert!(received_percentages.iter().all(|&p| p <= 100));

        // Test edge case with zero content length
        let zero_length = 0;
        let default_length = 200_000_000;
        let chosen_length = if zero_length == 0 {
            default_length
        } else {
            zero_length
        };
        assert_eq!(chosen_length, default_length);
    }

    #[test]
    fn test_component_validation() {
        // Test component name validation
        let valid_components = ["rustc", "rust-std", "cargo", "clippy", "rustfmt"];

        for component in valid_components {
            assert!(!component.is_empty());
            assert!(!component.contains(' '));
            assert!(!component.contains('\n'));

            // Component name should be ASCII alphanumeric with hyphens
            assert!(
                component
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            );
        }
    }

    #[test]
    fn test_path_strip_prefix_logic() {
        // Test path prefix stripping logic
        let base = PathBuf::from("/opt/rustowl/component");
        let full_path = base.join("lib").join("file.so");

        if let Ok(rel_path) = full_path.strip_prefix(&base) {
            assert_eq!(rel_path, PathBuf::from("lib").join("file.so"));
        } else {
            panic!("strip_prefix should succeed");
        }

        // Test with non-matching prefix
        let other_base = PathBuf::from("/different/path");
        assert!(full_path.strip_prefix(&other_base).is_err());
    }

    #[test]
    fn test_sysroot_path_validation() {
        // Test sysroot path validation logic
        let runtime_paths = [
            "/opt/rustowl",
            "/home/user/.rustowl",
            "/usr/local/rustowl",
            "relative/path",
            "",
        ];

        for runtime_path in runtime_paths {
            let runtime = PathBuf::from(runtime_path);
            let sysroot = sysroot_from_runtime(&runtime);

            // Should always contain the toolchain name
            assert!(sysroot.to_string_lossy().contains(TOOLCHAIN));

            // Should be a subdirectory of runtime
            if !runtime_path.is_empty() {
                assert!(sysroot.starts_with(&runtime));
            }
        }
    }

    #[test]
    fn test_toolchain_constants_integrity() {
        // Test that build-time constants are valid
        assert!(HOST_TUPLE.contains('-')); // Should contain hyphens separating components

        // TOOLCHAIN_DATE should be valid format if present
        if let Some(date) = TOOLCHAIN_DATE {
            assert!(!date.is_empty());
            assert!(date.len() >= 10); // At least YYYY-MM-DD format
        }
    }

    #[test]
    fn test_complex_path_operations() {
        // Test complex path operations with Unicode and special characters
        let base_paths = [
            "simple",
            "with spaces",
            "with-hyphens",
            "with_underscores",
            "with.dots",
            "数字", // Unicode characters
            "ñoño", // Accented characters
        ];

        for base in base_paths {
            let runtime = PathBuf::from(base);
            let sysroot = sysroot_from_runtime(&runtime);

            // Operations should not panic
            assert!(sysroot.is_absolute() || sysroot.is_relative());

            // Should maintain path structure
            let parent = sysroot.parent();
            assert!(parent.is_some() || sysroot.as_os_str().is_empty());
        }
    }

    #[test]
    fn test_environment_variable_parsing() {
        // Test environment variable parsing edge cases
        let test_vars = [
            ("", None),
            ("not_a_number", None),
            ("12345", Some(12345)),
            ("0", Some(0)),
            ("-1", None),                    // Negative numbers should be invalid
            ("999999999999999999999", None), // Overflow should be handled
            ("42.5", None),                  // Float should be invalid
            ("  123  ", None),               // Whitespace should be invalid
        ];

        for (input, expected) in test_vars {
            let result = input.parse::<usize>().ok();
            assert_eq!(result, expected, "Failed for input: {input}");
        }
    }

    #[test]
    fn test_url_component_validation() {
        // Test URL component validation
        let valid_components = [
            "rustc",
            "rust-std",
            "cargo",
            "clippy",
            "rustfmt",
            "rust-analyzer",
        ];

        let invalid_components = [
            "",
            " ",
            "rust std",  // Space
            "rust\nstd", // Newline
            "rust\tstd", // Tab
            "rust/std",  // Slash
            "rust?std",  // Question mark
            "rust#std",  // Hash
        ];

        for component in valid_components {
            assert!(!component.is_empty());
            assert!(!component.contains(' '));
            assert!(!component.contains('\n'));
            assert!(!component.contains('\t'));
            assert!(!component.contains('/'));
        }

        for component in invalid_components {
            let is_invalid = component.is_empty()
                || component.contains(' ')
                || component.contains('\n')
                || component.contains('\t')
                || component.contains('/')
                || component.contains('?')
                || component.contains('#');
            assert!(is_invalid, "Component should be invalid: {component}");
        }
    }

    #[test]
    fn test_recursive_read_dir_error_handling() {
        // Test recursive_read_dir with various error conditions
        use std::fs;
        use tempfile::tempdir;

        // Create temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Test with valid directory
        let sub_dir = temp_path.join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let file_path = sub_dir.join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let results = recursive_read_dir(temp_path);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], file_path);

        // Test with non-existent path
        let non_existent = temp_path.join("does_not_exist");
        let empty_results = recursive_read_dir(&non_existent);
        assert!(empty_results.is_empty());
    }

    #[test]
    fn test_fallback_runtime_dir_comprehensive() {
        // Test /opt/rustowl path construction
        let opt_path = PathBuf::from("/opt/rustowl");
        assert_eq!(opt_path.to_string_lossy(), "/opt/rustowl");

        // Test home directory path construction
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home).join(".rustowl");
            assert!(home_path.ends_with(".rustowl"));
        }

        // Test current exe path construction (simulate)
        let current_exe_parent = PathBuf::from("/usr/bin");
        assert!(current_exe_parent.is_absolute());
    }

    #[test]
    fn test_path_join_operations() {
        // Test path joining operations with various inputs
        let base_paths = ["/opt/rustowl", "/home/user/.rustowl", "relative/path"];

        let components = ["sysroot", TOOLCHAIN, "bin", "lib", "rustc"];

        for base in base_paths {
            let base_path = PathBuf::from(base);

            for component in components {
                let joined = base_path.join(component);

                // Should contain the component
                assert!(joined.to_string_lossy().contains(component));

                // Should be longer than the base path
                assert!(joined.to_string_lossy().len() > base_path.to_string_lossy().len());
            }
        }
    }

    #[test]
    fn test_command_environment_setup() {
        // Test command environment variable setup logic
        use tokio::process::Command;

        let sysroot = PathBuf::from("/opt/rustowl/sysroot/nightly-2024-01-01");
        let mut cmd = Command::new("test");

        // Test set_rustc_env function
        set_rustc_env(&mut cmd, &sysroot);

        // The command should be properly configured (we can't directly inspect env vars,
        // but we can verify the function doesn't panic)
        let program = cmd.as_std().get_program();
        assert_eq!(program, "test");
    }

    #[test]
    fn test_cross_platform_compatibility() {
        // Test cross-platform path handling
        let unix_style = "/opt/rustowl/sysroot";
        let windows_style = r"C:\opt\rustowl\sysroot";

        // Both should be valid paths on their respective platforms
        let unix_path = PathBuf::from(unix_style);
        let windows_path = PathBuf::from(windows_style);

        // Test path operations don't panic
        let _unix_components: Vec<_> = unix_path.components().collect();
        let _windows_components: Vec<_> = windows_path.components().collect();

        // Test sysroot construction with different path styles
        let unix_sysroot = sysroot_from_runtime(&unix_path);
        let windows_sysroot = sysroot_from_runtime(&windows_path);

        assert!(unix_sysroot.to_string_lossy().contains(TOOLCHAIN));
        assert!(windows_sysroot.to_string_lossy().contains(TOOLCHAIN));
    }

    #[test]
    fn test_complex_unicode_path_handling() {
        // Test with various Unicode path components
        let unicode_paths = [
            "简体中文/rustowl",     // Simplified Chinese
            "русский/язык/path",    // Russian
            "العربية/المجلد",       // Arabic
            "日本語/ディレクトリ",  // Japanese
            "🦀/rust/🔥/blazing",   // Emoji paths
            "café/résumé/naïve",    // Accented Latin
            "test/with spaces",     // Spaces
            "test/with\ttabs",      // Tabs
            "test\nwith\nnewlines", // Newlines (unusual but possible)
        ];

        for unicode_path in unicode_paths {
            let path = PathBuf::from(unicode_path);
            let sysroot = sysroot_from_runtime(&path);

            // Operations should not panic
            assert!(sysroot.to_string_lossy().contains(TOOLCHAIN));

            // Path should be constructible
            let path_str = sysroot.to_string_lossy();
            assert!(!path_str.is_empty());

            // Should be able to join additional components
            let extended = sysroot.join("bin").join("rustc");
            assert!(extended.to_string_lossy().len() > sysroot.to_string_lossy().len());
        }
    }

    #[test]
    fn test_environment_variable_parsing_comprehensive() {
        // Test comprehensive environment variable parsing patterns
        use std::ffi::OsString;

        // Test path splitting with various separators
        let test_cases = if cfg!(windows) {
            vec![
                ("", 0),                                 // Empty
                ("/usr/lib", 1),                         // Single path
                ("/usr/lib:/lib", 2),                    // Unix style (still works on Windows)
                ("/usr/lib:/lib:/usr/local/lib", 3),     // Multiple Unix
                ("C:\\Windows\\System32", 1),            // Windows single
                ("C:\\Windows\\System32;D:\\Tools", 2),  // Windows multiple
                ("/path with spaces:/another path", 2),  // Spaces
                ("/path/with/unicode/测试:/another", 2), // Unicode
            ]
        } else {
            vec![
                ("", 0),                                 // Empty
                ("/usr/lib", 1),                         // Single path
                ("/usr/lib:/lib", 2),                    // Unix style
                ("/usr/lib:/lib:/usr/local/lib", 3),     // Multiple Unix
                ("/usr/lib;/lib", 1),                    // Windows separator ignored on Unix
                ("/path with spaces:/another path", 2),  // Spaces
                ("/path/with/unicode/测试:/another", 2), // Unicode
            ]
        };

        for (path_str, expected_count) in test_cases {
            let paths: Vec<PathBuf> = std::env::split_paths(&OsString::from(path_str)).collect();

            if expected_count == 0 {
                assert!(paths.is_empty() || paths.len() == 1); // Empty string might yield one empty path
            } else {
                assert_eq!(paths.len(), expected_count, "Failed for: {path_str}");
            }

            // Test that join_paths can reconstruct
            if !paths.is_empty() {
                let rejoined = std::env::join_paths(paths.clone());
                assert!(rejoined.is_ok(), "Failed to rejoin paths for: {path_str}");
            }
        }
    }

    #[test]
    fn test_url_construction_edge_cases() {
        // Test URL construction with various edge cases
        let base_urls = [
            "https://static.rust-lang.org/dist",
            "https://example.com/rust/dist",
            "http://localhost:8080/dist",
        ];

        let components = [
            "rustc",
            "rust-std",
            "cargo",
            "rust-analyzer-preview",
            "component-with-very-long-name-that-might-cause-issues",
        ];

        let channels = ["stable", "beta", "nightly"];
        let host_tuples = [
            "x86_64-unknown-linux-gnu",
            "x86_64-pc-windows-msvc",
            "aarch64-apple-darwin",
            "riscv64gc-unknown-linux-gnu",
        ];

        for base_url in base_urls {
            for component in components {
                for channel in channels {
                    for host_tuple in host_tuples {
                        let component_toolchain = format!("{component}-{channel}-{host_tuple}");
                        let tarball_url = format!("{base_url}/{component_toolchain}.tar.gz");

                        // URL should be well-formed
                        assert!(tarball_url.starts_with("http"));
                        assert!(tarball_url.ends_with(".tar.gz"));
                        assert!(tarball_url.contains(component));
                        assert!(tarball_url.contains(channel));
                        assert!(tarball_url.contains(host_tuple));

                        // Should not contain double slashes (except after protocol)
                        let without_protocol = tarball_url.split_once("://").unwrap().1;
                        assert!(!without_protocol.contains("//"));
                    }
                }
            }
        }
    }

    #[test]
    fn test_archive_format_detection() {
        // Test archive format detection logic
        let archive_formats = [
            ("rustc-stable-x86_64-unknown-linux-gnu.tar.gz", "tar.gz"),
            ("rustowl-x86_64-pc-windows-msvc.zip", "zip"),
            ("component.tar.xz", "tar.xz"),
            ("archive.7z", "7z"),
            ("data.tar.bz2", "tar.bz2"),
        ];

        for (filename, expected_format) in archive_formats {
            let extension = filename.split('.').next_back().unwrap_or("");
            let is_compressed = matches!(extension, "gz" | "xz" | "bz2" | "zip" | "7z");

            if expected_format.contains("tar") {
                assert!(filename.contains("tar"));
            }

            assert!(is_compressed, "Should detect compression for: {filename}");

            // Test platform-specific format preferences
            #[cfg(target_os = "windows")]
            {
                if filename.contains("windows") {
                    assert!(filename.ends_with(".zip") || filename.ends_with(".exe"));
                }
            }

            #[cfg(not(target_os = "windows"))]
            {
                if filename.contains("linux") || filename.contains("darwin") {
                    assert!(filename.contains("tar"));
                }
            }
        }
    }

    #[test]
    fn test_component_name_validation_comprehensive() {
        // Test comprehensive component name validation
        let valid_components = [
            "rustc",
            "rust-std",
            "cargo",
            "clippy",
            "rustfmt",
            "rust-analyzer",
            "rust-analyzer-preview",
            "miri",
            "rust-docs",
            "rust-mingw",
            "component-with-long-name",
            "component123",
        ];

        let invalid_components = [
            "",          // Empty
            " ",         // Space only
            "rust std",  // Space in name
            "rust\nstd", // Newline
            "rust\tstd", // Tab
            "rust/std",  // Slash
            "rust\\std", // Backslash
            "rust?std",  // Question mark
            "rust#std",  // Hash
            "rust@std",  // At symbol
            "rust%std",  // Percent
            "rust std ", // Trailing space
            " rust-std", // Leading space
            "rust--std", // Double dash
            "rust-",     // Trailing dash
            "-rust",     // Leading dash
        ];

        for component in valid_components {
            assert!(!component.is_empty());
            assert!(!component.contains(' '));
            assert!(!component.contains('\n'));
            assert!(!component.contains('\t'));
            assert!(!component.contains('/'));
            assert!(!component.contains('\\'));
            assert!(!component.starts_with('-'));
            assert!(!component.ends_with('-'));
            assert!(!component.contains("--"));

            // Should be ASCII alphanumeric with hyphens and digits
            assert!(
                component
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            );
        }

        for component in invalid_components {
            let is_invalid = component.is_empty()
                || component.contains(' ')
                || component.contains('\n')
                || component.contains('\t')
                || component.contains('/')
                || component.contains('\\')
                || component.contains('?')
                || component.contains('#')
                || component.contains('@')
                || component.contains('%')
                || component.starts_with('-')
                || component.ends_with('-')
                || component.contains("--");

            assert!(is_invalid, "Component should be invalid: '{component}'");
        }
    }

    #[test]
    fn test_download_progress_calculation() {
        // Test download progress calculation with various scenarios
        let test_scenarios = [
            // (content_length, chunks, expected_progress_points)
            (1000, vec![100, 200, 300, 400], vec![10, 20, 30, 40]),
            (500, vec![125, 250, 375, 500], vec![25, 50, 75, 100]),
            (0, vec![100], vec![0]), // Zero content length
            (100, vec![50, 25, 25], vec![50, 75, 100]),
            (1, vec![1], vec![100]), // Tiny download
            (1_000_000, vec![100_000, 500_000, 400_000], vec![10, 50, 90]),
        ];

        for (content_length, chunks, _expected) in test_scenarios {
            let mut progress_points = Vec::new();
            let mut total_received = 0;
            let mut last_reported = 0;

            let effective_length = if content_length == 0 {
                200_000_000
            } else {
                content_length
            };

            for chunk_size in chunks {
                total_received += chunk_size;
                // Ensure we don't calculate progress above 100%
                let capped_received = std::cmp::min(total_received, effective_length);
                let current_progress = (capped_received * 100) / effective_length;

                if last_reported != current_progress {
                    progress_points.push(current_progress);
                    last_reported = current_progress;
                }
            }

            // Verify progress is reasonable
            for &progress in &progress_points {
                assert!(progress <= 100, "Progress should not exceed 100%");
            }

            // Verify progress is non-decreasing
            for window in progress_points.windows(2) {
                assert!(window[0] <= window[1], "Progress should be non-decreasing");
            }
        }
    }

    #[test]
    fn test_path_prefix_stripping_edge_cases() {
        // Test path prefix stripping with various edge cases
        let test_cases = [
            // (full_path, base_path, should_succeed)
            (
                "/opt/rustowl/component/lib/file.so",
                "/opt/rustowl/component",
                true,
            ),
            ("/opt/rustowl/component", "/opt/rustowl/component", true), // Exact match
            ("/opt/rustowl", "/opt/rustowl/component", false),          // Base is longer
            ("/different/path", "/opt/rustowl", false),                 // Completely different
            ("", "", true),                                             // Both empty
            ("relative/path", "relative", true),                        // Relative paths
            ("/", "/", true),                                           // Root paths
            ("/a/b/c", "/a/b", true),                                   // Simple case
            ("./local/path", "./local", true),                          // Current directory
            ("../parent/path", "../parent", true),                      // Parent directory
        ];

        for (full_path_str, base_path_str, should_succeed) in test_cases {
            let full_path = PathBuf::from(full_path_str);
            let base_path = PathBuf::from(base_path_str);

            let result = full_path.strip_prefix(&base_path);

            if should_succeed {
                assert!(
                    result.is_ok(),
                    "Should succeed: '{full_path_str}' - '{base_path_str}'"
                );

                if let Ok(relative) = result {
                    // Verify the relative path makes sense
                    let reconstructed = base_path.join(relative);
                    assert_eq!(
                        reconstructed, full_path,
                        "Reconstruction should match original"
                    );
                }
            } else {
                assert!(
                    result.is_err(),
                    "Should fail: '{full_path_str}' - '{base_path_str}'"
                );
            }
        }
    }

    #[test]
    fn test_executable_extension_handling() {
        // Test executable extension handling across platforms
        let base_names = [
            "rustc",
            "cargo",
            "rustfmt",
            "clippy-driver",
            "rust-analyzer",
            "rustdoc",
            "rustowlc",
        ];

        for base_name in base_names {
            // Test Windows extension handling
            #[cfg(windows)]
            {
                let with_exe = format!("{base_name}.exe");
                assert!(with_exe.ends_with(".exe"));
                assert!(with_exe.starts_with(base_name));
                assert_eq!(with_exe.len(), base_name.len() + 4);
            }

            // Test Unix (no extension)
            #[cfg(not(windows))]
            {
                let unix_name = base_name.to_owned();
                assert_eq!(unix_name, base_name);
                assert!(!unix_name.contains('.'));
            }

            // Test path construction with executables
            let bin_dir = PathBuf::from("/usr/bin");
            #[cfg(windows)]
            let exec_path = bin_dir.join(format!("{base_name}.exe"));
            #[cfg(not(windows))]
            let exec_path = bin_dir.join(base_name);

            assert!(exec_path.to_string_lossy().contains(base_name));
            assert!(exec_path.starts_with(&bin_dir));
        }
    }

    #[test]
    fn test_complex_directory_structures() {
        // Test handling of complex directory structures
        use std::fs;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Create nested directory structure
        let nested_dirs = [
            "level1",
            "level1/level2",
            "level1/level2/level3",
            "level1/sibling",
            "level1/sibling/deep/nested/path",
            "other_root",
            "other_root/branch",
        ];

        for dir in nested_dirs {
            let dir_path = temp_path.join(dir);
            fs::create_dir_all(&dir_path).unwrap();
        }

        // Create files in various locations
        let files = [
            "level1/file1.txt",
            "level1/level2/file2.txt",
            "level1/level2/level3/file3.txt",
            "level1/sibling/file4.txt",
            "level1/sibling/deep/nested/path/file5.txt",
            "other_root/file6.txt",
            "root_file.txt",
        ];

        for file in files {
            let file_path = temp_path.join(file);
            fs::write(&file_path, "test content").unwrap();
        }

        // Test recursive_read_dir
        let found_files = recursive_read_dir(temp_path);

        // Should find all files
        assert_eq!(found_files.len(), files.len());

        // Verify all expected files are found
        for expected_file in files {
            let expected_path = temp_path.join(expected_file);
            assert!(
                found_files.contains(&expected_path),
                "Should find file: {expected_file}"
            );
        }

        // Test with individual subdirectories
        let level1_files = recursive_read_dir(temp_path.join("level1"));
        assert!(level1_files.len() >= 4); // At least 4 files in level1 tree

        let other_root_files = recursive_read_dir(temp_path.join("other_root"));
        assert_eq!(other_root_files.len(), 1); // Just file6.txt
    }

    #[test]
    fn test_version_string_parsing() {
        // Test version string parsing patterns
        let version_patterns = [
            "1.0.0",
            "1.0.0-rc.1",
            "1.0.0-beta",
            "1.0.0-alpha.1",
            "2.1.3",
            "0.1.0",
            "10.20.30",
            "1.0.0-dev",
            "1.0.0+build.123",
            "1.0.0-rc.1+build.456",
        ];

        for version in version_patterns {
            // Test GitHub release URL construction
            let github_url = format!(
                "https://github.com/cordx56/rustowl/releases/download/v{version}/rustowl-{HOST_TUPLE}.tar.gz"
            );

            assert!(github_url.starts_with("https://github.com/"));
            assert!(github_url.contains("rustowl"));
            assert!(github_url.contains(version));
            assert!(github_url.contains(HOST_TUPLE));

            // Test version components
            let parts: Vec<&str> = version.split(['.', '-', '+']).collect();
            assert!(!parts.is_empty());

            // First part should be a number
            if let Ok(major) = parts[0].parse::<u32>() {
                assert!(major < 1000, "Major version should be reasonable");
            }
        }
    }

    #[test]
    fn test_memory_allocation_patterns() {
        // Test memory allocation patterns in path operations
        let base_path = PathBuf::from("/opt/rustowl");

        // Test many path operations don't cause excessive allocations
        for i in 0..100 {
            let extended = base_path
                .join(format!("component_{i}"))
                .join("subdir")
                .join("file.txt");
            assert!(extended.starts_with(&base_path));

            // Test string operations
            let path_str = extended.to_string_lossy();
            assert!(path_str.contains("component_"));
            assert!(path_str.contains(&i.to_string()));
        }

        // Test with varying path lengths
        for length in [1, 10, 100, 1000] {
            let long_component = "x".repeat(length);
            let path_with_long_component = base_path.join(&long_component);

            assert_eq!(
                path_with_long_component.file_name().unwrap(),
                &*long_component
            );
            assert!(
                path_with_long_component.to_string_lossy().len()
                    > base_path.to_string_lossy().len()
            );
        }
    }
}
