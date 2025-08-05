use std::env;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub fn is_incremental() -> bool {
    env::var("RUSTOWL_INCREMENTAL")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true)
}

pub fn set_incremental_path(cmd: &mut Command, target_dir: impl AsRef<Path>) {
    cmd.env(
        "RUSTOWL_INCREMENTAL_CACHE",
        target_dir.as_ref().join("incremental_cache.json"),
    );
}

pub fn get_incremental_path() -> Option<PathBuf> {
    env::var("RUSTOWL_INCREMENTAL_CACHE")
        .map(|v| PathBuf::from(v))
        .ok()
}
