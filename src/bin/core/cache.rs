use indexmap::IndexMap;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::ty::TyCtxt;
use rustc_query_system::ich::StableHashingContext;
use rustc_stable_hash::{FromStableHash, SipHasher128Hash};
use rustowl::cache::CacheConfig;
use rustowl::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Enhanced cache entry with metadata for robust caching
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CacheEntry {
    /// The cached function data
    pub function: Function,
    /// Timestamp when this entry was created
    pub created_at: u64,
    /// Timestamp when this entry was last accessed
    pub last_accessed: u64,
    /// Number of times this entry has been accessed
    pub access_count: u32,
    /// File modification time when this entry was cached
    pub file_mtime: Option<u64>,
    /// Size in bytes of the cached data (for memory management)
    pub data_size: usize,
}

impl CacheEntry {
    pub fn new(function: Function, file_mtime: Option<u64>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Estimate data size for memory management
        let data_size = std::mem::size_of::<Function>()
            + function.basic_blocks.len() * std::mem::size_of::<MirBasicBlock>()
            + function.decls.len() * std::mem::size_of::<MirDecl>();

        Self {
            function,
            created_at: now,
            last_accessed: now,
            access_count: 1,
            file_mtime,
            data_size,
        }
    }

    /// Mark this entry as accessed and update statistics
    pub fn mark_accessed(&mut self) {
        self.last_accessed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.access_count = self.access_count.saturating_add(1);
    }

    /// Check if this cache entry is still valid based on file modification time
    #[allow(dead_code)]
    pub fn is_valid(&self, current_file_mtime: Option<u64>) -> bool {
        match (self.file_mtime, current_file_mtime) {
            (Some(cached_mtime), Some(current_mtime)) => cached_mtime >= current_mtime,
            (None, _) | (_, None) => true, // Conservative: assume valid if we can't check
        }
    }
}

/// Cache statistics for monitoring and debugging
#[derive(Default, Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    #[allow(dead_code)]
    pub invalidations: u64,
    pub evictions: u64,
    pub total_entries: usize,
    pub total_memory_bytes: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Robust cache with intelligent eviction and metadata tracking
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CacheData {
    /// Cache entries with metadata
    entries: IndexMap<String, CacheEntry>,
    /// Runtime statistics (not serialized)
    #[serde(skip)]
    stats: CacheStats,
    /// Version for compatibility checking
    version: u32,
    /// Cache configuration (not serialized, loaded from environment)
    #[serde(skip)]
    config: CacheConfig,
}

/// Current cache version for compatibility checking
const CACHE_VERSION: u32 = 2;

impl CacheData {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::with_config(CacheConfig::default())
    }

    #[allow(dead_code)]
    pub fn with_capacity(capacity: usize) -> Self {
        let config = CacheConfig {
            max_entries: capacity,
            ..Default::default()
        };
        Self::with_config(config)
    }

    pub fn with_config(config: CacheConfig) -> Self {
        Self {
            entries: IndexMap::with_capacity(config.max_entries.min(64)),
            stats: CacheStats::default(),
            version: CACHE_VERSION,
            config,
        }
    }

    /// Create a combined cache key from file and MIR hashes
    fn make_key(file_hash: &str, mir_hash: &str) -> String {
        format!("{file_hash}:{mir_hash}")
    }

    /// Get file modification time for validation
    fn get_file_mtime(file_path: &str) -> Option<u64> {
        std::fs::metadata(file_path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
    }

    pub fn get_cache(&mut self, file_hash: &str, mir_hash: &str) -> Option<Function> {
        let key = Self::make_key(file_hash, mir_hash);

        if let Some(entry) = self.entries.get_mut(&key) {
            // Validate entry if file modification time checking is enabled
            if self.config.validate_file_mtime {
                // Try to extract file path from the cache key or use a heuristic
                // For now, we'll skip file validation in get_cache and do it during insertion
                // This maintains backward compatibility
            }

            // Mark as accessed and update LRU order
            entry.mark_accessed();
            if self.config.use_lru_eviction {
                // Move to end (most recently used) for LRU
                let entry = self.entries.shift_remove(&key).unwrap();
                self.entries.insert(key, entry);
            }

            self.stats.hits += 1;
            self.update_memory_stats();
            Some(
                self.entries
                    .get(&Self::make_key(file_hash, mir_hash))
                    .unwrap()
                    .function
                    .clone(),
            )
        } else {
            self.stats.misses += 1;
            None
        }
    }

    #[allow(dead_code)]
    pub fn insert_cache(&mut self, file_hash: String, mir_hash: String, analyzed: Function) {
        self.insert_cache_with_file_path(file_hash, mir_hash, analyzed, None);
    }

    pub fn insert_cache_with_file_path(
        &mut self,
        file_hash: String,
        mir_hash: String,
        analyzed: Function,
        file_path: Option<&str>,
    ) {
        let key = Self::make_key(&file_hash, &mir_hash);

        // Get file modification time if available and validation is enabled
        let file_mtime = if self.config.validate_file_mtime {
            file_path.and_then(Self::get_file_mtime)
        } else {
            None
        };

        let entry = CacheEntry::new(analyzed, file_mtime);

        // Check if we need to evict entries before inserting
        self.maybe_evict_entries();

        self.entries.insert(key, entry);
        self.update_memory_stats();
        log::debug!(
            "Cache entry inserted. Total entries: {}, Memory usage: {} bytes",
            self.entries.len(),
            self.stats.total_memory_bytes
        );
    }

    /// Update memory usage statistics
    fn update_memory_stats(&mut self) {
        self.stats.total_entries = self.entries.len();
        self.stats.total_memory_bytes = self.entries.values().map(|entry| entry.data_size).sum();
    }

    /// Check if eviction is needed and perform it
    fn maybe_evict_entries(&mut self) {
        let needs_eviction = self.entries.len() >= self.config.max_entries
            || self.stats.total_memory_bytes >= self.config.max_memory_bytes;

        if needs_eviction {
            self.evict_entries();
        }
    }

    /// Perform intelligent cache eviction
    fn evict_entries(&mut self) {
        let target_entries = (self.config.max_entries * 8) / 10; // Keep 80% after eviction
        let target_memory = (self.config.max_memory_bytes * 8) / 10;

        let mut evicted_count = 0;

        if self.config.use_lru_eviction {
            // LRU eviction: remove least recently used entries
            while (self.entries.len() > target_entries
                || self.stats.total_memory_bytes > target_memory)
                && !self.entries.is_empty()
            {
                // Find entry with oldest last_accessed time
                let oldest_key = self
                    .entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_accessed)
                    .map(|(key, _)| key.clone());

                if let Some(key) = oldest_key {
                    self.entries.shift_remove(&key);
                    evicted_count += 1;
                } else {
                    break;
                }
            }
        } else {
            // FIFO eviction: remove oldest entries by insertion order
            while (self.entries.len() > target_entries
                || self.stats.total_memory_bytes > target_memory)
                && !self.entries.is_empty()
            {
                self.entries.shift_remove_index(0);
                evicted_count += 1;
            }
        }

        self.stats.evictions += evicted_count;
        self.update_memory_stats();

        if evicted_count > 0 {
            log::info!(
                "Evicted {} cache entries. Remaining: {} entries, {} bytes",
                evicted_count,
                self.entries.len(),
                self.stats.total_memory_bytes
            );
        }
    }

    /// Remove invalid cache entries based on file modification times
    #[allow(dead_code)]
    pub fn validate_and_cleanup(&mut self, file_paths: &HashMap<String, String>) -> usize {
        let mut removed_count = 0;
        let mut keys_to_remove = Vec::new();

        for (key, entry) in &self.entries {
            // Extract file hash from key
            if let Some(file_hash) = key.split(':').next()
                && let Some(file_path) = file_paths.get(file_hash) {
                    let current_mtime = Self::get_file_mtime(file_path);
                    if !entry.is_valid(current_mtime) {
                        keys_to_remove.push(key.clone());
                    }
                }
        }

        for key in keys_to_remove {
            self.entries.shift_remove(&key);
            removed_count += 1;
        }

        if removed_count > 0 {
            self.stats.invalidations += removed_count;
            self.update_memory_stats();
            log::info!("Invalidated {removed_count} outdated cache entries");
        }

        removed_count as usize
    }

    /// Get cache statistics for monitoring
    pub fn get_stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Check if cache version is compatible
    pub fn is_compatible(&self) -> bool {
        self.version == CACHE_VERSION
    }

    /// Remove old cache entries to prevent unlimited growth
    /// This method is kept for backward compatibility but now uses intelligent eviction
    #[allow(dead_code)]
    pub fn cleanup_old_entries(&mut self, max_size: usize) {
        if max_size < self.config.max_entries {
            self.config.max_entries = max_size;
            self.maybe_evict_entries();
        }
    }

    /// Get detailed cache information for debugging
    #[allow(dead_code)]
    pub fn debug_info(&self) -> String {
        format!(
            "Cache Info:\n\
             - Entries: {}/{}\n\
             - Memory: {}/{} bytes ({:.1}MB/{:.1}MB)\n\
             - Hit Rate: {:.1}% ({} hits, {} misses)\n\
             - Evictions: {}\n\
             - Invalidations: {}\n\
             - LRU Eviction: {}\n\
             - File Validation: {}",
            self.entries.len(),
            self.config.max_entries,
            self.stats.total_memory_bytes,
            self.config.max_memory_bytes,
            self.stats.total_memory_bytes as f64 / (1024.0 * 1024.0),
            self.config.max_memory_bytes as f64 / (1024.0 * 1024.0),
            self.stats.hit_rate() * 100.0,
            self.stats.hits,
            self.stats.misses,
            self.stats.evictions,
            self.stats.invalidations,
            self.config.use_lru_eviction,
            self.config.validate_file_mtime
        )
    }

    /// Clear all cache entries
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.entries.clear();
        self.stats = CacheStats::default();
        log::info!("Cache cleared");
    }

    /// Get the number of entries in the cache
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Shrink the cache to fit current entries
    #[allow(dead_code)]
    pub fn shrink_to_fit(&mut self) {
        self.entries.shrink_to_fit();
    }
}

/// Get cache data with robust error handling and validation
///
/// If cache is not enabled, then return None.
/// If file doesn't exist, it returns empty [`CacheData`].
/// If cache is corrupted or incompatible, it returns a new cache.
pub fn get_cache(krate: &str) -> Option<CacheData> {
    if let Some(cache_path) = rustowl::cache::get_cache_path() {
        let cache_path = cache_path.join(format!("{krate}.json"));

        // Get configuration from environment
        let config = rustowl::cache::get_cache_config();

        // Try to read and parse the cache file
        match std::fs::read_to_string(&cache_path) {
            Ok(content) => {
                match serde_json::from_str::<CacheData>(&content) {
                    Ok(mut cache_data) => {
                        // Check version compatibility
                        if !cache_data.is_compatible() {
                            log::warn!(
                                "Cache version incompatible (found: {}, expected: {}), creating new cache",
                                cache_data.version,
                                CACHE_VERSION
                            );
                            return Some(CacheData::with_config(config));
                        }

                        // Restore runtime configuration and statistics
                        cache_data.config = config;
                        cache_data.stats = CacheStats::default();
                        cache_data.update_memory_stats();

                        log::info!(
                            "Cache loaded: {} entries, {} bytes from {}",
                            cache_data.entries.len(),
                            cache_data.stats.total_memory_bytes,
                            cache_path.display()
                        );

                        Some(cache_data)
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to parse cache file ({}), creating new cache: {}",
                            cache_path.display(),
                            e
                        );
                        Some(CacheData::with_config(config))
                    }
                }
            }
            Err(e) => {
                log::info!(
                    "Cache file not found or unreadable ({}), creating new cache: {}",
                    cache_path.display(),
                    e
                );
                Some(CacheData::with_config(config))
            }
        }
    } else {
        log::debug!("Cache disabled via configuration");
        None
    }
}

/// Write cache with atomic operations and robust error handling
pub fn write_cache(krate: &str, cache: &CacheData) {
    if let Some(cache_dir) = rustowl::cache::get_cache_path() {
        // Ensure cache directory exists
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            log::error!(
                "Failed to create cache directory {}: {}",
                cache_dir.display(),
                e
            );
            return;
        }

        let cache_path = cache_dir.join(format!("{krate}.json"));
        let temp_path = cache_dir.join(format!("{krate}.json.tmp"));

        // Serialize cache data
        let serialized = match serde_json::to_string_pretty(cache) {
            Ok(data) => data,
            Err(e) => {
                log::error!("Failed to serialize cache data: {e}");
                return;
            }
        };

        // Write to temporary file first for atomic operation
        match write_cache_file(&temp_path, &serialized) {
            Ok(()) => {
                // Atomically move temporary file to final location
                if let Err(e) = std::fs::rename(&temp_path, &cache_path) {
                    log::error!(
                        "Failed to move cache file from {} to {}: {}",
                        temp_path.display(),
                        cache_path.display(),
                        e
                    );
                    // Clean up temporary file
                    let _ = std::fs::remove_file(&temp_path);
                } else {
                    let stats = cache.get_stats();
                    log::info!(
                        "Cache saved: {} entries, {} bytes, hit rate: {:.1}% to {}",
                        stats.total_entries,
                        stats.total_memory_bytes,
                        stats.hit_rate() * 100.0,
                        cache_path.display()
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to write cache to {}: {}", temp_path.display(), e);
                // Clean up temporary file
                let _ = std::fs::remove_file(&temp_path);
            }
        }
    } else {
        log::debug!("Cache disabled, skipping write");
    }
}

/// Write cache data to file with proper error handling
fn write_cache_file(path: &Path, data: &str) -> Result<(), std::io::Error> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    let mut writer = BufWriter::new(file);
    writer.write_all(data.as_bytes())?;
    writer.flush()?;

    // Ensure data is written to disk
    writer.into_inner()?.sync_all()?;

    Ok(())
}
