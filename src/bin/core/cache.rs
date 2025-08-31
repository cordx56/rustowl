use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::ty::TyCtxt;
use rustc_query_system::ich::StableHashingContext;
use rustc_stable_hash::{FromStableHash, SipHasher128Hash};
use rustowl::models::*;
use serde::{Deserialize, Serialize};
use indexmap::IndexMap;
use std::io::Write;
use std::sync::{LazyLock, Mutex};

pub static CACHE: LazyLock<Mutex<Option<CacheData>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Debug, Clone)]
struct StableHashString(String);
impl StableHashString {
    pub fn get(self) -> String {
        self.0
    }
}
impl FromStableHash for StableHashString {
    type Hash = SipHasher128Hash;
    fn from(hash: Self::Hash) -> Self {
        let byte0 = hash.0[0] as u128;
        let byte1 = hash.0[1] as u128;
        let byte = (byte0 << 64) | byte1;
        Self(format!("{byte:x}"))
    }
}

pub struct Hasher<'a> {
    hasher: StableHasher,
    hash_ctx: StableHashingContext<'a>,
}

impl<'tcx> Hasher<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            hasher: StableHasher::default(),
            hash_ctx: StableHashingContext::new(tcx.sess, tcx.untracked()),
        }
    }

    fn finish(self) -> String {
        self.hasher.finish::<StableHashString>().get()
    }

    pub fn get_hash(
        tcx: TyCtxt<'tcx>,
        target: impl HashStable<StableHashingContext<'tcx>>,
    ) -> String {
        let mut new = Self::new(tcx);
        target.hash_stable(&mut new.hash_ctx, &mut new.hasher);
        new.finish()
    }
}

/// Optimized cache using a flattened structure with combined keys
/// This reduces memory overhead and improves cache performance
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct CacheData(IndexMap<String, Function>);

impl CacheData {
    pub fn new() -> Self {
        Self(IndexMap::with_capacity(64))
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        Self(IndexMap::with_capacity(capacity))
    }
    
    /// Create a combined cache key from file and MIR hashes
    fn make_key(file_hash: &str, mir_hash: &str) -> String {
        format!("{file_hash}:{mir_hash}")
    }
    
    pub fn get_cache(&self, file_hash: &str, mir_hash: &str) -> Option<Function> {
        let key = Self::make_key(file_hash, mir_hash);
        self.0.get(&key).cloned()
    }
    
    pub fn insert_cache(&mut self, file_hash: String, mir_hash: String, analyzed: Function) {
        let key = Self::make_key(&file_hash, &mir_hash);
        self.0.insert(key, analyzed);
    }
    
    /// Remove old cache entries to prevent unlimited growth
    pub fn cleanup_old_entries(&mut self, max_size: usize) {
        if self.0.len() > max_size {
            let to_remove = self.0.len() - max_size;
            // Remove oldest entries (first in IndexMap)
            for _ in 0..to_remove {
                self.0.shift_remove_index(0);
            }
        }
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
                return Some(CacheData::new());
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
