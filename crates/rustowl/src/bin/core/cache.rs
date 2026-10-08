use rustowl::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

pub static CACHE: LazyLock<Mutex<Option<CacheData>>> = LazyLock::new(|| Mutex::new(None));

/// One body as recorded in the `by_built` index.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedBody {
    pub def_id: u32,
    pub file_hash: String,
    pub mir_hash: String,
    pub file_path: PathBuf,
}

/// Incremental analysis cache.
///
/// `by_file` holds the payloads, keyed file hash then MIR body hash. `by_built`
/// indexes them by the hash of `tcx.mir_built`, which is available *without*
/// running borrow check, so a warm root can be answered without paying for a
/// second `get_borrowck_facts`.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct CacheData {
    pub by_file: HashMap<String, HashMap<String, Function>>,
    #[serde(default)]
    pub by_built: HashMap<String, Vec<CachedBody>>,
}
impl CacheData {
    pub fn get_cache(&self, file_hash: &str, mir_hash: &str) -> Option<Function> {
        self.by_file
            .get(file_hash)
            .and_then(|v| v.get(mir_hash))
            .cloned()
    }
    pub fn insert_cache(&mut self, file_hash: String, mir_hash: String, analyzed: Function) {
        self.by_file
            .entry(file_hash)
            .or_default()
            .insert(mir_hash, analyzed);
    }
    pub fn index_built(&mut self, built_hash: String, body: CachedBody) {
        let entry = self.by_built.entry(built_hash).or_default();
        if !entry.iter().any(|b| b.def_id == body.def_id) {
            entry.push(body);
        }
    }
    pub fn bodies_for(&self, built_hash: &str) -> Option<&Vec<CachedBody>> {
        self.by_built.get(built_hash)
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
        // a cache written in an older format is discarded, so that it is
        // rebuilt instead of disabling the cache
        let read = serde_json::from_str(&s).unwrap_or_else(|e| {
            log::warn!("discard incompatible incremental cache file: {e}");
            CacheData::default()
        });
        log::debug!("cache read: {}", cache_path.display());
        Some(read)
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
        log::debug!("incremental cache saved: {}", cache_path.display());
    }
}
