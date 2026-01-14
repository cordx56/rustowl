use rustowl::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{LazyLock, Mutex};

pub static CACHE: LazyLock<Mutex<Option<CacheData>>> = LazyLock::new(|| Mutex::new(None));

/// Single file cache body
///
/// this is a map: file hash -> (MIR body hash -> analyze result)
///
/// Note: Cache can be utilized when neither
/// the MIR body nor the entire file is modified.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(transparent)]
pub struct CacheData(HashMap<String, HashMap<String, Function>>);
impl CacheData {
    pub fn get_cache(&self, file_hash: &str, mir_hash: &str) -> Option<Function> {
        self.0.get(file_hash).and_then(|v| v.get(mir_hash)).cloned()
    }
    pub fn insert_cache(&mut self, file_hash: String, mir_hash: String, analyzed: Function) {
        self.0
            .entry(file_hash)
            .or_default()
            .insert(mir_hash, analyzed);
    }
}

/// Get cache data
///
/// If cache is not enabled, then return None.
/// If file is not exists, it returns empty [`CacheData`].
pub fn get_cache(krate: &str) -> Option<CacheData> {
    if let Some(cache_path) = rustowl::cache::get_cache_path() {
        let cache_path = cache_path.join(format!("{krate}.json"));
        let s = match std::fs::read_to_string(&cache_path) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("failed to read incremental cache file: {e}");
                return Some(CacheData::default());
            }
        };
        let read = serde_json::from_str(&s).ok();
        log::info!("cache read: {}", cache_path.display());
        read
    } else {
        None
    }
}

pub fn write_cache(krate: &str, cache: &CacheData) {
    if let Some(cache_path) = rustowl::cache::get_cache_path() {
        if let Err(e) = std::fs::create_dir_all(&cache_path) {
            log::warn!("failed to create cache dir: {e}");
            return;
        }
        let cache_path = cache_path.join(format!("{krate}.json"));
        let s = serde_json::to_string(cache).unwrap();
        let mut f = match std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&cache_path)
        {
            Ok(v) => v,
            Err(e) => {
                log::warn!("failed to open incremental cache file: {e}");
                return;
            }
        };
        if let Err(e) = f.write_all(s.as_bytes()) {
            log::warn!("failed to write incremental cache file: {e}");
        }
        log::info!("incremental cache saved: {}", cache_path.display());
    }
}
