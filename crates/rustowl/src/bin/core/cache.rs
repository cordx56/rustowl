pub mod key;

use arc_swap::ArcSwap;
use archery::{ArcK, SharedPointer};
use rustowl::models::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use tinyufo::TinyUfo;

/// Bumped when the meaning of a cached payload changes.
///
/// 2: the store is keyed by the hash of the body's identity, and an entry
/// carries that identity, so a hash collision is a miss rather than a wrong
/// body.
pub const ANALYSIS_VERSION: u32 = 2;

/// In-memory weight budget, in TinyUfo's 4 KiB clusters.
const WEIGHT_BUDGET: usize = 65_536;

/// The ghost queue keeps admission history for evicted keys.
const GHOST_CAPACITY: usize = 65_536;

pub const ENTRIES_FILE: &str = "entries.json";

pub struct Cache {
    ufo: TinyUfo<u64, SharedPointer<key::CachedEntry, ArcK>>,
    /// Storage keys written this run; a hit-only run leaves it empty and writes nothing.
    touched: ArcSwap<rpds::HashTrieSet<u64, ArcK>>,
}

/// Hash of the running rustc version, or `0` if it cannot be identified.
///
/// `version_meta` shells out to the compiler, so it is computed once per process.
static TOOLCHAIN_HASH: OnceLock<u64> = OnceLock::new();

pub fn toolchain_hash() -> u64 {
    *TOOLCHAIN_HASH.get_or_init(|| {
        use std::hash::{Hash, Hasher};
        match rustc_version::version_meta() {
            Ok(meta) => {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                meta.semver.hash(&mut hasher);
                hasher.finish()
            }
            Err(e) => {
                tracing::warn!("could not identify the running rustc: {e}");
                0
            }
        }
    })
}

static CACHE: OnceLock<Option<Cache>> = OnceLock::new();

pub fn cache() -> Option<&'static Cache> {
    CACHE.get().and_then(Option::as_ref)
}

/// A store from another rustowl or toolchain gets its own path, so it is inert.
fn store_path(root: &std::path::Path, krate: &str) -> PathBuf {
    root.join(format!("{krate}.v{ANALYSIS_VERSION}.t{}", toolchain_hash()))
}

/// The entry's weight in TinyUfo's 4 KiB clusters.
fn weight(entry: &key::CachedEntry) -> u16 {
    entry.estimated_bytes().div_ceil(4 * 1024).clamp(1, 1024) as u16
}

/// The JSON file is the durable store; TinyUfo only bounds how much of it stays hot.
fn open_cache(krate: &str) -> Option<Cache> {
    let root = rustowl::cache::get_cache_path()?;
    let store = store_path(&root, krate);
    let tag = store.file_name()?.to_str()?.rsplit_once('.')?.1.to_owned();

    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name.starts_with(&format!("{krate}.")) && !name.ends_with(&tag) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    std::fs::create_dir_all(&store).ok()?;

    let stored: HashMap<String, key::CachedEntry> = std::fs::read(store.join(ENTRIES_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let ufo = TinyUfo::new(WEIGHT_BUDGET, GHOST_CAPACITY);
    for (sk, entry) in stored {
        // A key that is not a hash belongs to an older format: it is dropped.
        let Ok(hash) = u64::from_str_radix(&sk, 16) else {
            continue;
        };
        let w = weight(&entry);
        ufo.force_put(hash, SharedPointer::new(entry), w);
    }
    Some(Cache {
        ufo,
        touched: ArcSwap::from_pointee(rpds::HashTrieSet::default()),
    })
}

pub fn init_cache(krate: &str) {
    let opened = open_cache(krate);
    if opened.is_some() {
        tracing::debug!("incremental cache opened for {krate}");
    }
    let _ = CACHE.set(opened);
}

/// The full identity of a cached body, as it is written to disk.
fn storage_key(key: &key::FunctionKey) -> String {
    format!(
        "{:x}:{:x}:{}:{}:{:x}",
        key.def_path, key.fn_text_hash, key.mir_hash, key.analysis_version, key.toolchain_hash
    )
}

/// The hash of the full identity: what the in-memory cache is keyed by, so a
/// lookup builds no string.
fn storage_hash(key: &key::FunctionKey) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.def_path.hash(&mut hasher);
    key.fn_text_hash.hash(&mut hasher);
    key.mir_hash.hash(&mut hasher);
    key.analysis_version.hash(&mut hasher);
    key.toolchain_hash.hash(&mut hasher);
    hasher.finish()
}

/// Look up a cached body. The entry's own identity is checked against the
/// request, so a hash collision is a miss rather than a wrong body.
pub fn get_entry(key: &key::FunctionKey) -> Option<SharedPointer<key::CachedEntry, ArcK>> {
    let entry = cache()?.ufo.get(&storage_hash(key))?;
    if entry.key != storage_key(key) {
        return None;
    }
    Some(entry)
}

pub fn put_entry(
    key: &key::FunctionKey,
    base: rustowl::models::Loc,
    analyzed: SharedPointer<Function, ArcK>,
) {
    let Some(cache) = cache() else { return };
    let hash = storage_hash(key);
    let entry = SharedPointer::new(key::CachedEntry {
        base,
        function: analyzed,
        key: storage_key(key),
    });
    cache
        .ufo
        .force_put(hash, SharedPointer::clone(&entry), weight(&entry));
    let mut touched = cache.touched.load().as_ref().clone();
    touched = touched.insert(hash);
    cache.touched.store(Arc::new(touched));
}

/// Persist bodies recorded this run.
///
/// The file is read back and merged, so hit bodies are never rewritten and
/// bodies analysed in earlier runs stay stored.
pub fn persist_entries(krate: &str) {
    let Some(root) = rustowl::cache::get_cache_path() else {
        return;
    };
    let Some(cache) = cache() else {
        return;
    };
    let store = store_path(&root, krate);
    if !store.is_dir() {
        return;
    }
    let touched = cache.touched.load();
    if touched.is_empty() {
        return;
    }
    let path = store.join(ENTRIES_FILE);
    let mut entries: HashMap<String, SharedPointer<key::CachedEntry, ArcK>> = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut stored = 0;
    for sk in touched.iter() {
        // An entry evicted this run is not rewritten; its disk copy is still good.
        if let Some(entry) = cache.ufo.get(sk) {
            entries.insert(format!("{sk:x}"), entry);
            stored += 1;
        }
    }
    if let Err(e) = std::fs::write(
        &path,
        serde_json::to_string(&entries).expect("entries must serialize"),
    ) {
        tracing::warn!("failed to write the cached bodies: {e}");
    } else {
        tracing::debug!("persisted {stored} newly analysed bodies for {krate}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_path_carries_the_version_and_toolchain() {
        let name = store_path(std::path::Path::new("/cache"), "my_crate");
        let name = name.file_name().unwrap().to_str().unwrap().to_owned();

        assert!(name.starts_with("my_crate.v"), "{name}");
        assert!(
            name.contains(&toolchain_hash().to_string()),
            "{name} must carry the toolchain identity"
        );
    }

    #[test]
    fn no_cache_dir_means_no_cache() {
        assert!(
            open_cache("test_crate").is_none(),
            "a cache must not be built without RUSTOWL_CACHE_DIR"
        );
    }

    #[test]
    fn weight_is_bounded_in_clusters() {
        let small = key::CachedEntry {
            base: Loc(0),
            function: SharedPointer::new(Function {
                fn_id: 1,
                name: "f".into(),
                basic_blocks: vec![],
                decls: vec![],
            }),
            key: String::new(),
        };
        let huge = key::CachedEntry {
            base: Loc(0),
            function: SharedPointer::new(Function {
                fn_id: 1,
                name: "a".repeat(6 * 1024 * 1024),
                basic_blocks: vec![],
                decls: vec![],
            }),
            key: String::new(),
        };

        assert_eq!(
            weight(&small),
            1,
            "the smallest body still weighs 1 cluster"
        );
        assert_eq!(
            weight(&huge),
            1024,
            "a body larger than 4 MiB must not overflow u16 weights"
        );
    }
}
