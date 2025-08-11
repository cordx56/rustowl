use std::env;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub fn is_cache() -> bool {
    !env::var("RUSTOWL_CACHE")
        .map(|v| v == "false" || v == "0")
        .unwrap_or(false)
}

pub fn set_cache_path(cmd: &mut Command, target_dir: impl AsRef<Path>) {
    cmd.env("RUSTOWL_CACHE_DIR", target_dir.as_ref().join("cache"));
}

pub fn get_cache_path() -> Option<PathBuf> {
    env::var("RUSTOWL_CACHE_DIR").map(PathBuf::from).ok()
}
