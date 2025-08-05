use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::ty::TyCtxt;
use rustc_query_system::ich::StableHashingContext;
use rustc_stable_hash::{FromStableHash, SipHasher128Hash};
use rustowl::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;

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
        let byte = (byte0 << 64) & byte1;
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
            hash_ctx: StableHashingContext::new(&tcx.sess, tcx.untracked()),
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

/// Single file cache body
///
/// this is a map: file hash -> (MIR body hash -> analyze result)
///
/// Note: Cache can be utilized when neither
/// the MIR body nor the entire file is modified.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct CacheData(HashMap<String, HashMap<String, Function>>);
impl CacheData {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn get_cache(&self, file_hash: &str, mir_hash: &str) -> Option<Function> {
        self.0
            .get(file_hash)
            .map(|v| v.get(mir_hash))
            .flatten()
            .cloned()
    }
    pub fn insert_cache(&mut self, file_hash: String, mir_hash: String, analyzed: Function) {
        self.0
            .entry(file_hash)
            .or_insert(HashMap::new())
            .insert(mir_hash, analyzed);
    }
}

/// Incremental hash file is a map.
/// MIR body hash -> analyzed result.
pub fn get_incremental_cache() -> Option<CacheData> {
    if let Some(cache_path) = rustowl::cache::get_incremental_path() {
        let s = match std::fs::read_to_string(&cache_path) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("failed to read incremental cache file: {e}");
                return None;
            }
        };
        serde_json::from_str(&s).ok()
    } else {
        None
    }
}

pub fn write_incremental_cache(cache: &CacheData) {
    if let Some(cache_path) = rustowl::cache::get_incremental_path() {
        let s = serde_json::to_string(cache).unwrap();
        let mut f = match std::fs::OpenOptions::new()
            .write(true)
            .create(true)
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
    }
}
