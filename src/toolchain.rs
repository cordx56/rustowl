use std::env;
use std::io::Read;
use std::time::Duration;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use tar::{Archive, EntryType};

use tokio::fs::OpenOptions;
use tokio::fs::{create_dir_all, read_to_string, remove_dir_all, rename};
use tokio::io::AsyncWriteExt;

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

fn sysroot_looks_installed(sysroot: &Path) -> bool {
    let rustc = if cfg!(windows) { "rustc.exe" } else { "rustc" };
    let cargo = if cfg!(windows) { "cargo.exe" } else { "cargo" };

    sysroot.join("bin").join(rustc).is_file()
        && sysroot.join("bin").join(cargo).is_file()
        && sysroot.join("lib").is_dir()
}

async fn get_runtime_dir() -> PathBuf {
    let sysroot = sysroot_from_runtime(&*FALLBACK_RUNTIME_DIR);
    if FALLBACK_RUNTIME_DIR.is_dir() && sysroot_looks_installed(&sysroot) {
        return FALLBACK_RUNTIME_DIR.clone();
    }

    tracing::debug!("sysroot not found (or incomplete); start setup toolchain");
    if let Err(e) = setup_toolchain(&*FALLBACK_RUNTIME_DIR, false).await {
        tracing::error!("{e:?}");
        std::process::exit(1);
    }

    FALLBACK_RUNTIME_DIR.clone()
}

pub async fn get_sysroot() -> PathBuf {
    if let Ok(override_path) = env::var("RUSTOWL_SYSROOT") {
        let override_path = PathBuf::from(override_path);
        if override_path.is_dir() {
            return override_path;
        }
    }

    sysroot_from_runtime(get_runtime_dir().await)
}

const DOWNLOAD_CAP_BYTES: u64 = 2_000_000_000;

#[derive(Clone, Copy, Debug)]
struct DownloadCaps {
    max_download: u64,
    max_retries: usize,
    retry_backoff: Duration,
}

impl DownloadCaps {
    const DEFAULT: Self = Self {
        max_download: DOWNLOAD_CAP_BYTES,
        max_retries: 5,
        retry_backoff: Duration::from_millis(250),
    };
}

fn hash_url_for_filename(url: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn spool_dir_for_runtime(runtime: &Path) -> PathBuf {
    runtime.join(".rustowl-cache").join("downloads")
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn hash_url_for_filename_is_stable_and_hex() {
        let url = "https://example.com/archive.tar.gz";
        let a = hash_url_for_filename(url);
        let b = hash_url_for_filename(url);
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
    }

    #[test]
    fn spool_dir_is_under_runtime_cache() {
        let runtime = PathBuf::from("/tmp/rustowl-runtime");
        assert_eq!(
            spool_dir_for_runtime(&runtime),
            runtime.join(".rustowl-cache").join("downloads")
        );
    }

    #[test]
    fn extracted_components_are_staged_under_spool_dir() {
        let spool = Path::new("/home/user/.rustowl/.rustowl-cache/downloads");
        assert_eq!(extract_base_dir_for_spool(spool), spool.join("extract"));
    }

    #[test]
    fn sysroot_from_runtime_uses_toolchain_component() {
        let runtime = PathBuf::from("/opt/rustowl");
        assert_eq!(
            sysroot_from_runtime(&runtime),
            runtime.join("sysroot").join(TOOLCHAIN)
        );
    }

    #[test]
    fn sysroot_looks_installed_checks_expected_layout() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sysroot = tmp.path().join("sysroot");
        std::fs::create_dir_all(sysroot.join("bin")).unwrap();
        std::fs::create_dir_all(sysroot.join("lib")).unwrap();

        let rustc = if cfg!(windows) { "rustc.exe" } else { "rustc" };
        let cargo = if cfg!(windows) { "cargo.exe" } else { "cargo" };

        assert!(!sysroot_looks_installed(&sysroot));

        std::fs::write(sysroot.join("bin").join(rustc), "").unwrap();
        assert!(!sysroot_looks_installed(&sysroot));

        std::fs::write(sysroot.join("bin").join(cargo), "").unwrap();
        assert!(sysroot_looks_installed(&sysroot));
    }

    #[test]
    fn safe_join_tar_path_rejects_escape_attempts() {
        let dest = Path::new("/safe/root");
        assert!(safe_join_tar_path(dest, Path::new("../evil")).is_err());
        assert!(safe_join_tar_path(dest, Path::new("/abs/path")).is_err());

        let ok = safe_join_tar_path(dest, Path::new("dir/file.txt")).expect("ok");
        assert_eq!(ok, dest.join("dir").join("file.txt"));
    }

    #[test]
    fn unpack_tarball_gz_skips_symlinks() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use tar::Builder;

        let temp = tempfile::tempdir().expect("tempdir");
        let dest = temp.path().join("out");
        std::fs::create_dir_all(&dest).unwrap();

        let mut tar_buf = Vec::new();
        {
            let gz = GzEncoder::new(&mut tar_buf, Compression::default());
            let mut builder = Builder::new(gz);

            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_cksum();
            builder
                .append_data(&mut header, "symlink", std::io::empty())
                .unwrap();

            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(4);
            header.set_cksum();
            builder
                .append_data(&mut header, "dir/file.txt", "data".as_bytes())
                .unwrap();

            let gz = builder.into_inner().unwrap();
            gz.finish().unwrap();
        }

        unpack_tarball_gz(std::io::Cursor::new(tar_buf), &dest).expect("unpack ok");

        let extracted = dest.join("dir").join("file.txt");
        assert!(extracted.exists());
        assert_eq!(std::fs::read_to_string(extracted).unwrap(), "data");

        assert!(!dest.join("symlink").exists());
    }

    #[test]
    fn safe_join_tar_path_rejects_empty_and_dot_only_paths() {
        let dest = Path::new("/safe/root");
        assert!(safe_join_tar_path(dest, Path::new(".")).is_err());
        assert!(safe_join_tar_path(dest, Path::new("././.")).is_err());
        assert!(safe_join_tar_path(dest, Path::new("")).is_err());

        let ok = safe_join_tar_path(dest, Path::new("./dir/./file.txt")).expect("ok");
        assert_eq!(ok, dest.join("dir").join("file.txt"));
    }

    #[test]
    fn unpack_tarball_gz_rejects_path_traversal_entry() {
        // The tar crate itself rejects `..` and absolute paths at archive-build time,
        // so we can't construct those invalid entries via `tar::Builder`.
        // Instead, we validate `safe_join_tar_path` directly for those cases.
        let dest = Path::new("/safe/root");
        assert!(safe_join_tar_path(dest, Path::new("../evil.txt")).is_err());
        assert!(safe_join_tar_path(dest, Path::new("/evil.txt")).is_err());
    }
}

async fn download_with_resume(
    url: &str,
    spool_path: &Path,
    caps: DownloadCaps,
    progress: Option<indicatif::ProgressBar>,
) -> Result<(), ()> {
    static HTTP_CLIENT: std::sync::LazyLock<reqwest::Client> =
        std::sync::LazyLock::new(reqwest::Client::new);

    let mut existing = match tokio::fs::metadata(spool_path).await {
        Ok(meta) => meta.len(),
        Err(_) => 0,
    };

    if let Some(pb) = &progress {
        pb.set_position(existing);
        pb.set_message("Downloading...".to_string());
    }

    tracing::debug!(
        "downloading {url} into {} (resume from {existing})",
        spool_path.display()
    );

    // If we have a partial spool, validate Range support before resuming.
    let mut resp = if existing > 0 {
        let r = HTTP_CLIENT
            .get(url)
            .header(reqwest::header::RANGE, format!("bytes={existing}-"))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("failed to download runtime archive");
                tracing::error!("{e:?}");
            })?;

        match r.status() {
            reqwest::StatusCode::PARTIAL_CONTENT => r,
            // Some servers respond 416 when the local file is already complete.
            reqwest::StatusCode::RANGE_NOT_SATISFIABLE => {
                tracing::debug!("range not satisfiable; treating spool as complete");
                if let Some(pb) = &progress {
                    pb.set_position(existing);
                }
                return Ok(());
            }
            // Server ignored range; start fresh.
            reqwest::StatusCode::OK => {
                tracing::debug!("server did not honor range; restarting download");
                existing = 0;
                let _ = tokio::fs::remove_file(spool_path).await;
                HTTP_CLIENT
                    .get(url)
                    .send()
                    .await
                    .and_then(|v| v.error_for_status())
                    .map_err(|e| {
                        tracing::error!("failed to download runtime archive");
                        tracing::error!("{e:?}");
                    })?
            }

            other => {
                tracing::error!("unexpected HTTP status for range request: {other}");
                return Err(());
            }
        }
    } else {
        HTTP_CLIENT
            .get(url)
            .send()
            .await
            .and_then(|v| v.error_for_status())
            .map_err(|e| {
                tracing::error!("failed to download runtime archive");
                tracing::error!("{e:?}");
            })?
    };

    let mut downloaded = existing;

    loop {
        let expected_total = match (downloaded, resp.content_length()) {
            (0, Some(v)) => Some(v),
            (n, Some(v)) => Some(n.saturating_add(v)),
            _ => None,
        };
        if matches!(expected_total, Some(v) if v > caps.max_download) {
            tracing::error!("refusing to download {url}: size exceeds cap");
            return Err(());
        }

        if let (Some(pb), Some(total)) = (progress.as_ref(), expected_total) {
            pb.set_length(total);
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(spool_path)
            .await
            .map_err(|e| {
                tracing::error!("failed to open download file {}: {e}", spool_path.display());
            })?;

        match stream_response_body(
            url,
            &mut resp,
            &mut file,
            &mut downloaded,
            caps,
            progress.as_ref(),
        )
        .await
        {
            Ok(()) => {
                file.flush().await.ok();
                tracing::debug!("download finished: {} bytes", downloaded);
                return Ok(());
            }
            Err(()) => {
                // Retry loop: request from current offset.
                // If this fails `max_retries` times, we error out.
                let mut attempt = 1usize;
                loop {
                    if attempt > caps.max_retries {
                        tracing::error!("download failed after {} retries", caps.max_retries);
                        return Err(());
                    }

                    tokio::time::sleep(caps.retry_backoff * attempt as u32).await;
                    tracing::debug!("retrying download from byte {downloaded} (attempt {attempt})");

                    let r = HTTP_CLIENT
                        .get(url)
                        .header(reqwest::header::RANGE, format!("bytes={downloaded}-"))
                        .send()
                        .await
                        .and_then(|v| v.error_for_status());

                    match r {
                        Ok(v) if v.status() == reqwest::StatusCode::PARTIAL_CONTENT => {
                            resp = v;
                            break;
                        }
                        Ok(v) if v.status() == reqwest::StatusCode::OK => {
                            tracing::debug!(
                                "server ignored resume range; restarting download from 0"
                            );
                            downloaded = 0;
                            let _ = tokio::fs::remove_file(spool_path).await;
                            resp = HTTP_CLIENT
                                .get(url)
                                .send()
                                .await
                                .and_then(|v| v.error_for_status())
                                .map_err(|e| {
                                    tracing::error!("failed to download runtime archive");
                                    tracing::error!("{e:?}");
                                })?;
                            break;
                        }
                        Ok(v) => {
                            tracing::error!("server did not honor resume range: {}", v.status());
                            return Err(());
                        }
                        Err(e) => {
                            tracing::debug!("retry request failed: {e:?}");
                            attempt += 1;
                            continue;
                        }
                    }
                }

                // Continue outer loop with new response.
                continue;
            }
        }
    }
}

async fn stream_response_body(
    url: &str,
    resp: &mut reqwest::Response,
    file: &mut tokio::fs::File,
    downloaded: &mut u64,
    caps: DownloadCaps,
    progress: Option<&indicatif::ProgressBar>,
) -> Result<(), ()> {
    loop {
        let chunk = match resp.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(e) => {
                // Transient HTTP/2 resets happen in the wild (e.g. CDN/proxy).
                // Treat as retryable so the caller can resume via Range.
                tracing::debug!("failed to read download chunk: {e:?}");
                return Err(());
            }
        };

        *downloaded = downloaded.saturating_add(chunk.len() as u64);
        if *downloaded > caps.max_download {
            tracing::error!("refusing to download {url}: exceeded size cap");
            return Err(());
        }

        file.write_all(&chunk).await.map_err(|e| {
            tracing::error!("failed writing download chunk: {e}");
        })?;

        if let Some(pb) = progress {
            pb.set_position(*downloaded);
        }
    }

    Ok(())
}

fn safe_join_tar_path(dest: &Path, path: &Path) -> Result<PathBuf, ()> {
    use std::path::Component;

    let mut out = dest.to_path_buf();
    let mut pushed_any = false;

    for component in path.components() {
        match component {
            Component::Normal(part) => {
                out.push(part);
                pushed_any = true;
            }
            Component::CurDir => continue,
            _ => return Err(()),
        }
    }

    if !pushed_any {
        return Err(());
    }

    Ok(out)
}

fn unpack_tarball_gz(reader: impl std::io::Read, dest: &Path) -> Result<(), ()> {
    // basic DoS protection
    const MAX_ENTRY_UNCOMPRESSED: u64 = DOWNLOAD_CAP_BYTES;
    const MAX_TOTAL_UNCOMPRESSED: u64 = DOWNLOAD_CAP_BYTES;

    let decoder = GzDecoder::new(reader);
    let mut archive = Archive::new(decoder);

    let mut total_uncompressed = 0u64;
    for entry in archive.entries().map_err(|_| ())? {
        let mut entry = entry.map_err(|_| ())?;

        let entry_type = entry.header().entry_type();
        match entry_type {
            EntryType::Regular | EntryType::Directory => {}
            // Be conservative: skip symlinks/hardlinks/devices.
            _ => {
                continue;
            }
        }

        let path = entry.path().map_err(|_| ())?;
        let out_path = safe_join_tar_path(dest, &path).map_err(|_| ())?;

        #[cfg(unix)]
        let mode = entry.header().mode().ok();

        if entry_type == EntryType::Directory {
            std::fs::create_dir_all(&out_path).map_err(|_| ())?;
            #[cfg(unix)]
            if let Some(mode) = mode {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(mode));
            }
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| ())?;
        }

        let mut out = std::fs::File::create(&out_path).map_err(|_| ())?;
        let mut limited = (&mut entry).take(MAX_ENTRY_UNCOMPRESSED.saturating_add(1));
        let written = std::io::copy(&mut limited, &mut out).map_err(|_| ())?;

        if written > MAX_ENTRY_UNCOMPRESSED {
            return Err(());
        }

        #[cfg(unix)]
        if let Some(mode) = mode {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(mode));
        }

        total_uncompressed = total_uncompressed.saturating_add(written);
        if total_uncompressed > MAX_TOTAL_UNCOMPRESSED {
            return Err(());
        }
    }

    Ok(())
}

async fn download_tarball_and_extract(
    url: &str,
    dest: &Path,
    spool_dir: &Path,
    progress: Option<indicatif::ProgressBar>,
) -> Result<(), ()> {
    create_dir_all(spool_dir).await.map_err(|e| {
        tracing::error!("failed to create spool dir {}: {e}", spool_dir.display());
    })?;

    if let Some(pb) = &progress {
        pb.set_message("Downloading...".to_string());
    }

    let archive_path = spool_dir.join(format!("{}.tar.gz", hash_url_for_filename(url)));

    download_with_resume(url, &archive_path, DownloadCaps::DEFAULT, progress.clone()).await?;

    if let Some(pb) = &progress {
        pb.set_message("Extracting...".to_string());
    }

    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path).map_err(|_| ())?;
        unpack_tarball_gz(file, &dest)
    })
    .await
    .map_err(|e| {
        tracing::error!("failed to join unpack task: {e}");
    })?
    .map_err(|_| {
        tracing::error!("failed to unpack tarball");
    })?;

    if let Some(pb) = progress {
        pb.finish_with_message("Installed");
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn safe_join_zip_path(dest: &Path, filename: &str) -> Result<PathBuf, ()> {
    use std::path::Component;

    let path = Path::new(filename);
    let mut out = dest.to_path_buf();
    let mut pushed_any = false;

    for component in path.components() {
        match component {
            Component::Normal(part) => {
                out.push(part);
                pushed_any = true;
            }
            Component::CurDir => continue,
            _ => return Err(()),
        }
    }

    if !pushed_any {
        return Err(());
    }

    Ok(out)
}

#[cfg(target_os = "windows")]
async fn download_zip_and_extract(
    url: &str,
    dest: &Path,
    spool_dir: &Path,
    progress: Option<indicatif::ProgressBar>,
) -> Result<(), ()> {
    create_dir_all(spool_dir).await.map_err(|e| {
        tracing::error!("failed to create spool dir {}: {e}", spool_dir.display());
    })?;

    if let Some(pb) = &progress {
        pb.set_message("Downloading...".to_string());
    }

    let archive_path = spool_dir.join(format!("{}.zip", hash_url_for_filename(url)));

    download_with_resume(url, &archive_path, DownloadCaps::DEFAULT, progress.clone()).await?;

    if let Some(pb) = &progress {
        pb.set_message("Extracting...".to_string());
    }

    let archive_path = archive_path.to_path_buf();
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || {
        use std::io::{Read as _, Write as _};

        // basic DoS protection
        const MAX_ENTRY_UNCOMPRESSED: u64 = DOWNLOAD_CAP_BYTES;
        const MAX_TOTAL_UNCOMPRESSED: u64 = DOWNLOAD_CAP_BYTES;

        let file = std::fs::File::open(&archive_path).map_err(|e| {
            tracing::error!("failed to open zip {}: {e}", archive_path.display());
        })?;
        let reader = std::io::BufReader::new(file);

        let mut zip = zip::ZipArchive::new(reader).map_err(|e| {
            tracing::error!("failed to read zip archive: {e}");
        })?;

        let mut total_uncompressed = 0u64;

        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| {
                tracing::error!("failed reading zip entry: {e}");
            })?;

            let name = entry.name().to_string();
            let out_path = safe_join_zip_path(&dest, &name)?;

            if name.ends_with('/') {
                std::fs::create_dir_all(&out_path).map_err(|e| {
                    tracing::error!("failed creating dir {}: {e}", out_path.display());
                })?;
                continue;
            }

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    tracing::error!("failed creating parent dir {}: {e}", parent.display());
                })?;
            }

            // Guard against maliciously large entries.
            let mut written_for_entry = 0u64;
            let mut out = std::fs::File::create(&out_path).map_err(|e| {
                tracing::error!("failed creating file {}: {e}", out_path.display());
            })?;

            let mut buf = [0u8; 32 * 1024];
            loop {
                let n = entry.read(&mut buf).map_err(|e| {
                    tracing::error!("failed reading zip data: {e}");
                })?;
                if n == 0 {
                    break;
                }

                written_for_entry = written_for_entry.saturating_add(n as u64);
                if written_for_entry > MAX_ENTRY_UNCOMPRESSED {
                    tracing::error!("zip entry exceeds size cap");
                    return Err(());
                }

                total_uncompressed = total_uncompressed.saturating_add(n as u64);
                if total_uncompressed > MAX_TOTAL_UNCOMPRESSED {
                    tracing::error!("zip total exceeds size cap");
                    return Err(());
                }

                out.write_all(&buf[..n]).map_err(|e| {
                    tracing::error!("failed writing zip data: {e}");
                })?;
            }

            // Ensure we fully consume any remaining compressed data and land on a sane boundary.
            let _ = entry.seek(std::io::SeekFrom::Current(0));
        }

        Ok::<(), ()>(())
    })
    .await
    .map_err(|e| {
        tracing::error!("failed to join unpack task: {e}");
    })?
    .map_err(|_| {
        tracing::error!("failed to unpack zip");
    })?;

    if let Some(pb) = progress {
        pb.finish_with_message("Installed");
    }

    Ok(())
}

struct ExtractedComponent {
    _tempdir: tempfile::TempDir,
    extracted_root: PathBuf,
}

fn extract_base_dir_for_spool(spool_dir: &Path) -> PathBuf {
    spool_dir.join("extract")
}

async fn fetch_component(
    component: &str,
    base_url: &str,
    spool_dir: &Path,
    progress: Option<indicatif::ProgressBar>,
) -> Result<ExtractedComponent, ()> {
    // Avoid using OS temp directories (often tmpfs) because toolchain components
    // are large and can quickly exhaust memory-backed storage.
    let temp_path = extract_base_dir_for_spool(spool_dir);
    if create_dir_all(&temp_path).await.is_err() {
        tracing::error!("failed to create extraction directory");
        return Err(());
    }

    let tempdir = tempfile::Builder::new()
        .prefix("rustowl-extract-")
        .tempdir_in(&temp_path)
        .map_err(|_| ())?;
    let temp_path = tempdir.path().to_owned();
    tracing::debug!("temp dir is made: {}", temp_path.display());

    let component_toolchain = format!("{component}-{TOOLCHAIN_CHANNEL}-{HOST_TUPLE}");
    let tarball_url = format!("{base_url}/{component_toolchain}.tar.gz");

    download_tarball_and_extract(&tarball_url, &temp_path, spool_dir, progress).await?;

    Ok(ExtractedComponent {
        _tempdir: tempdir,
        extracted_root: temp_path.join(component_toolchain),
    })
}

async fn install_extracted_component(extracted: ExtractedComponent, dest: &Path) -> Result<(), ()> {
    let components = read_to_string(extracted.extracted_root.join("components"))
        .await
        .map_err(|_| {
            tracing::error!("failed to read components list");
        })?;
    let components = components.split_whitespace();

    for component_name in components {
        let component_path = extracted.extracted_root.join(component_name);
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
                // This is expected when temp directories are on a different device (EXDEV).
                tracing::debug!("file rename failed: {e}, falling back to copy and delete");
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
        tracing::debug!("component {component_name} successfully installed");
    }
    Ok(())
}

pub async fn setup_toolchain(dest: impl AsRef<Path>, skip_rustowl: bool) -> Result<(), ()> {
    if skip_rustowl {
        setup_rust_toolchain(&dest).await
    } else {
        tokio::try_join!(setup_rust_toolchain(&dest), setup_rustowl_toolchain(&dest)).map(|_| ())
    }
}

pub async fn setup_rust_toolchain(dest: impl AsRef<Path>) -> Result<(), ()> {
    use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
    use std::io::IsTerminal;

    let sysroot = sysroot_from_runtime(dest.as_ref());
    if create_dir_all(&sysroot).await.is_err() {
        tracing::error!("failed to create toolchain directory");
        return Err(());
    }

    let dist_base = "https://static.rust-lang.org/dist";
    let base_url = match TOOLCHAIN_DATE {
        Some(v) => format!("{dist_base}/{v}"),
        None => dist_base.to_owned(),
    };

    tracing::debug!("start installing Rust toolchain...");

    const COMPONENTS: [&str; 3] = ["rustc", "rust-std", "cargo"];

    let spool_dir = spool_dir_for_runtime(dest.as_ref());

    let mp = if std::io::stderr().is_terminal() {
        Some(MultiProgress::with_draw_target(ProgressDrawTarget::stderr()))
    } else {
        None
    };

    // Ensure `tracing` output is routed through a progress bar so it doesn't
    // corrupt the multi-progress rendering.
    let _log_guard = mp.as_ref().map(|mp| {
        let pb = mp.add(ProgressBar::hidden());
        crate::ActiveProgressBarGuard::set(pb)
    });

    let mut fetched = HashMap::<&'static str, ExtractedComponent>::new();
    let mut set = tokio::task::JoinSet::new();

    for component in COMPONENTS {
        let base_url = base_url.clone();
        let spool_dir = spool_dir.clone();

        let pb: Option<ProgressBar> = mp.as_ref().map(|mp| {
            let pb = mp.add(ProgressBar::new(0));
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} {prefix:8} {msg:40} [{bar:40.cyan/blue}] {percent:>3}% ({bytes_per_sec:>10}, {eta:>6})",
                )
                .unwrap(),
            );
            pb.set_prefix(component.to_string());
            pb.set_message("Starting...".to_string());
            pb
        });

        set.spawn(async move {
            let res = fetch_component(component, &base_url, &spool_dir, pb.clone()).await;
            if let Some(pb) = pb {
                match &res {
                    Ok(_) => pb.finish_with_message("Installed"),
                    Err(_) => pb.finish_with_message("Failed"),
                }
            }
            (component, res)
        });
    }

    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((component, Ok(extracted))) => {
                fetched.insert(component, extracted);
            }
            Ok((_component, Err(()))) => {
                if let Some(mp) = &mp {
                    let _ = mp.clear();
                }
                return Err(());
            }
            Err(e) => {
                tracing::error!("failed to join toolchain fetch task: {e}");
                if let Some(mp) = &mp {
                    let _ = mp.clear();
                }
                return Err(());
            }
        }
    }

    let rustc = fetched.remove("rustc").ok_or(())?;
    let rust_std = fetched.remove("rust-std").ok_or(())?;
    let cargo = fetched.remove("cargo").ok_or(())?;

    install_extracted_component(rustc, &sysroot).await?;
    install_extracted_component(rust_std, &sysroot).await?;
    install_extracted_component(cargo, &sysroot).await?;

    if let Some(mp) = mp {
        let _ = mp.clear();
    }

    tracing::debug!("installing Rust toolchain finished");
    Ok(())
}

pub async fn setup_rustowl_toolchain(dest: impl AsRef<Path>) -> Result<(), ()> {
    tracing::debug!("start installing RustOwl toolchain...");

    let spool_dir = spool_dir_for_runtime(dest.as_ref());

    #[cfg(not(target_os = "windows"))]
    let rustowl_toolchain_result = {
        let rustowl_tarball_url = format!(
            "https://github.com/cordx56/rustowl/releases/download/v{}/rustowl-{HOST_TUPLE}.tar.gz",
            clap::crate_version!(),
        );
        download_tarball_and_extract(&rustowl_tarball_url, dest.as_ref(), &spool_dir, None).await
    };

    #[cfg(target_os = "windows")]
    let rustowl_toolchain_result = {
        let rustowl_zip_url = format!(
            "https://github.com/cordx56/rustowl/releases/download/v{}/rustowl-{HOST_TUPLE}.zip",
            clap::crate_version!(),
        );
        download_zip_and_extract(&rustowl_zip_url, dest.as_ref(), &spool_dir, None).await
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

    // Allow overriding specific tool paths for dev/bench setups.
    // Example: `RUSTOWL_RUSTOWLC_PATH=/path/to/rustowlc`.
    let override_key = format!("RUSTOWL_{}_PATH", name.to_ascii_uppercase());
    if let Ok(path) = env::var(&override_key) {
        let path = PathBuf::from(path);
        if path.is_file() {
            tracing::debug!("{name} is selected via {override_key}");
            return path.to_string_lossy().to_string();
        }
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

    // When running benches/tests, the binary might live in `target/{debug,release}`
    // while the current executable is in `target/{debug,release}/deps`.
    if let Ok(cwd) = env::current_dir() {
        let candidate = cwd.join("target").join("debug").join(&exec_name);
        if candidate.is_file() {
            tracing::debug!("{name} is selected in target/debug");
            return candidate.to_string_lossy().to_string();
        }

        let candidate = cwd.join("target").join("release").join(&exec_name);
        if candidate.is_file() {
            tracing::debug!("{name} is selected in target/release");
            return candidate.to_string_lossy().to_string();
        }
    }

    tracing::warn!("{name} not found; fallback");
    exec_name.to_owned()
}

pub async fn setup_cargo_command(rustc_threads: usize) -> tokio::process::Command {
    let cargo = get_executable_path("cargo").await;
    let mut command = tokio::process::Command::new(&cargo);
    let rustowlc = get_executable_path("rustowlc").await;

    // check user set flags
    let delimiter = 0x1f as char;
    let rustflags = env::var("RUSTFLAGS")
        .unwrap_or("".to_string())
        .split_whitespace()
        .fold("".to_string(), |acc, x| format!("{acc}{delimiter}{x}"));
    let mut encoded_flags = env::var("CARGO_ENCODED_RUSTFLAGS")
        .map(|v| format!("{v}{delimiter}"))
        .unwrap_or("".to_string());
    if 1 < rustc_threads {
        encoded_flags = format!("-Z{delimiter}threads={rustc_threads}{delimiter}{encoded_flags}");
    }

    let sysroot = get_sysroot().await;
    command
        .env("RUSTC", &rustowlc)
        .env("RUSTC_WORKSPACE_WRAPPER", &rustowlc)
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
    command.env("RUSTC_BOOTSTRAP", "1"); // Support nightly projects

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
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn test_sysroot_from_runtime() {
        let runtime = PathBuf::from("/opt/test-runtime");
        let sysroot = sysroot_from_runtime(&runtime);

        let expected = runtime.join("sysroot").join(TOOLCHAIN);
        assert_eq!(sysroot, expected);
    }

    #[test]
    fn set_rustc_env_sets_bootstrap_and_sysroot_flags() {
        let sysroot = PathBuf::from("/opt/rust/sysroot");
        let mut cmd = tokio::process::Command::new("cargo");
        set_rustc_env(&mut cmd, &sysroot);

        let envs: BTreeMap<String, String> = cmd
            .as_std()
            .get_envs()
            .filter_map(|(key, value)| {
                Some((
                    key.to_string_lossy().to_string(),
                    value?.to_string_lossy().to_string(),
                ))
            })
            .collect();

        assert_eq!(envs.get("RUSTC_BOOTSTRAP").map(String::as_str), Some("1"));

        #[cfg(target_os = "linux")]
        {
            let lib = sysroot.join("lib").to_string_lossy().to_string();
            assert!(
                envs.get("LD_LIBRARY_PATH")
                    .is_some_and(|v| v.contains(lib.as_str()))
            );
        }
        #[cfg(target_os = "macos")]
        {
            let lib = sysroot.join("lib").to_string_lossy().to_string();
            assert!(
                envs.get("DYLD_FALLBACK_LIBRARY_PATH")
                    .is_some_and(|v| v.contains(lib.as_str()))
            );
        }
        #[cfg(target_os = "windows")]
        {
            let bin = sysroot.join("bin").to_string_lossy().to_string();
            assert!(envs.get("Path").is_some_and(|v| v.contains(bin.as_str())));
        }
    }

    use crate::miri_async_test;

    #[test]
    fn setup_cargo_command_encodes_threads_and_sysroot() {
        miri_async_test!(async {
            let sysroot = get_sysroot().await;
            let cmd = setup_cargo_command(4).await;

            let envs: BTreeMap<String, String> = cmd
                .as_std()
                .get_envs()
                .filter_map(|(key, value)| {
                    Some((
                        key.to_string_lossy().to_string(),
                        value?.to_string_lossy().to_string(),
                    ))
                })
                .collect();

            assert_eq!(
                envs.get("RUSTC_WORKSPACE_WRAPPER").map(String::as_str),
                envs.get("RUSTC").map(String::as_str)
            );

            let encoded = envs
                .get("CARGO_ENCODED_RUSTFLAGS")
                .expect("CARGO_ENCODED_RUSTFLAGS set by setup_cargo_command");
            assert!(encoded.contains("-Z\u{1f}threads=4\u{1f}"));
            assert!(encoded.contains(&format!("--sysroot={}", sysroot.display())));

            assert_eq!(envs.get("RUSTC_BOOTSTRAP").map(String::as_str), Some("1"));
        });
    }

    #[test]
    fn setup_cargo_command_preserves_user_rustflags_in_encoded_string() {
        let delimiter = 0x1f as char;

        let user_rustflags = "-C debuginfo=2";
        let rustflags = user_rustflags
            .split_whitespace()
            .fold(String::new(), |acc, x| format!("{acc}{delimiter}{x}"));

        let user_encoded = "--cfg".to_owned() + &delimiter.to_string() + "from_user";
        let mut encoded_flags = format!("{user_encoded}{delimiter}");

        let rustc_threads = 4;
        if 1 < rustc_threads {
            encoded_flags =
                format!("-Z{delimiter}threads={rustc_threads}{delimiter}{encoded_flags}");
        }

        let sysroot = PathBuf::from("/opt/rust/sysroot");
        let mut cmd = tokio::process::Command::new("cargo");
        cmd.env(
            "CARGO_ENCODED_RUSTFLAGS",
            format!(
                "{}--sysroot={}{}",
                encoded_flags,
                sysroot.display(),
                rustflags
            ),
        );

        let envs: BTreeMap<String, String> = cmd
            .as_std()
            .get_envs()
            .filter_map(|(key, value)| {
                Some((
                    key.to_string_lossy().to_string(),
                    value?.to_string_lossy().to_string(),
                ))
            })
            .collect();

        let encoded = envs.get("CARGO_ENCODED_RUSTFLAGS").unwrap();
        assert!(encoded.contains("--cfg\u{1f}from_user\u{1f}"));
        assert!(encoded.contains("\u{1f}-C\u{1f}debuginfo=2"));
    }
}
