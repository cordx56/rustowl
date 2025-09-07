use indexmap::IndexMap;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::ty::TyCtxt;
use rustc_query_system::ich::StableHashingContext;
use rustc_stable_hash::{FromStableHash, SipHasher128Hash};
use rustowl::cache::CacheConfig;
use rustowl::models::*;
use serde::{Deserialize, Serialize};
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
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Estimate data size via serialization to capture heap usage
        let data_size = serde_json::to_vec(&function).map(|v| v.len()).unwrap_or(0);

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
            .map(|d| d.as_secs())
            .unwrap_or(self.last_accessed);
        self.access_count = self.access_count.saturating_add(1);
    }
}

/// Cache statistics for monitoring and debugging
#[derive(Default, Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub invalidations: u64, // file-change-based removals
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

    pub fn get_cache(
        &mut self,
        file_hash: &str,
        mir_hash: &str,
        file_path: Option<&str>,
    ) -> Option<Function> {
        let key = Self::make_key(file_hash, mir_hash);

        if self.config.use_lru_eviction {
            if let Some(mut entry) = self.entries.shift_remove(&key) {
                // Validate file modification time if file path is provided and validation is enabled
                if let Some(file_path) = file_path
                    && self.config.validate_file_mtime
                    && let Some(cached_mtime) = entry.file_mtime
                    && let Some(current_mtime) = Self::get_file_mtime(file_path)
                    && current_mtime > cached_mtime
                {
                    // File has been modified since caching, invalidate this entry
                    tracing::debug!(
                        "Cache entry invalidated due to file modification: {}",
                        file_path
                    );
                    self.stats.invalidations += 1;
                    self.update_memory_stats();
                    self.stats.misses += 1;
                    return None;
                }

                entry.mark_accessed();
                let function = entry.function.clone();
                self.entries.insert(key, entry);
                self.update_memory_stats();

                // Evict if needed after reinsertion to prevent temporary overshoot
                self.maybe_evict_entries();

                self.stats.hits += 1;
                return Some(function);
            }
        } else {
            // First, determine if the entry should be invalidated without holding a mutable borrow across removal
            let should_invalidate = if let Some(entry) = self.entries.get(&key) {
                if let Some(file_path) = file_path
                    && self.config.validate_file_mtime
                    && let Some(cached_mtime) = entry.file_mtime
                    && let Some(current_mtime) = Self::get_file_mtime(file_path)
                    && current_mtime > cached_mtime
                {
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if should_invalidate {
                tracing::debug!("Cache entry invalidated due to file modification: {:?}", file_path);
                self.entries.swap_remove(&key);
                self.stats.invalidations += 1;
                self.update_memory_stats();
                self.stats.misses += 1;
                return None;
            }

            // Normal hit path
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.mark_accessed();
                self.stats.hits += 1;
                return Some(entry.function.clone());
            }
        }
        self.stats.misses += 1;
        None
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

        // Evict again after insertion to prevent temporary overshoot
        self.maybe_evict_entries();

        tracing::debug!(
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
        let target_entries = ((self.config.max_entries * 8) / 10).max(1); // Keep >=1 entry
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
                    .map(|(key, _)| key);

                if let Some(key) = oldest_key {
                    // Clone the key only when we need to remove it
                    let key_to_remove = key.clone();
                    if let Some(removed) = self.entries.shift_remove(&key_to_remove) {
                        self.stats.total_memory_bytes = self
                            .stats
                            .total_memory_bytes
                            .saturating_sub(removed.data_size);
                        evicted_count += 1;
                    }
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
                if let Some((_, removed)) = self.entries.shift_remove_index(0) {
                    self.stats.total_memory_bytes = self
                        .stats
                        .total_memory_bytes
                        .saturating_sub(removed.data_size);
                    evicted_count += 1;
                }
            }
        }

        self.stats.evictions += evicted_count;
        self.update_memory_stats();

        if evicted_count > 0 {
            tracing::info!(
                "Evicted {} cache entries. Remaining: {} entries, {} bytes",
                evicted_count,
                self.entries.len(),
                self.stats.total_memory_bytes
            );
        }
    }

    /// Get cache statistics for monitoring
    pub fn get_stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Check if cache version is compatible
    pub fn is_compatible(&self) -> bool {
        self.version == CACHE_VERSION
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
                            tracing::warn!(
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

                        tracing::info!(
                            "Cache loaded: {} entries, {} bytes from {}",
                            cache_data.entries.len(),
                            cache_data.stats.total_memory_bytes,
                            cache_path.display()
                        );

                        Some(cache_data)
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse cache file ({}), creating new cache: {}",
                            cache_path.display(),
                            e
                        );
                        Some(CacheData::with_config(config))
                    }
                }
            }
            Err(e) => {
                tracing::info!(
                    "Cache file not found or unreadable ({}), creating new cache: {}",
                    cache_path.display(),
                    e
                );
                Some(CacheData::with_config(config))
            }
        }
    } else {
        tracing::debug!("Cache disabled via configuration");
        None
    }
}

/// Write cache with atomic operations and robust error handling
pub fn write_cache(krate: &str, cache: &CacheData) {
    if let Some(cache_dir) = rustowl::cache::get_cache_path() {
        // Ensure cache directory exists
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            tracing::error!(
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
                tracing::error!("Failed to serialize cache data: {e}");
                return;
            }
        };

        // Write to temporary file first for atomic operation
        match write_cache_file(&temp_path, &serialized) {
            Ok(()) => {
                // Atomically move temporary file to final location
                if let Err(e) = std::fs::rename(&temp_path, &cache_path) {
                    tracing::error!(
                        "Failed to move cache file from {} to {}: {}",
                        temp_path.display(),
                        cache_path.display(),
                        e
                    );
                    // Clean up temporary file
                    let _ = std::fs::remove_file(&temp_path);
                } else {
                    let stats = cache.get_stats();
                    tracing::info!(
                        "Cache saved: {} entries, {} bytes, hit rate: {:.1}%, evictions: {}, invalidations: {} to {}",
                        stats.total_entries,
                        stats.total_memory_bytes,
                        stats.hit_rate() * 100.0,
                        stats.evictions,
                        stats.invalidations,
                        cache_path.display()
                    );
                }
            }
            Err(e) => {
                tracing::error!("Failed to write cache to {}: {}", temp_path.display(), e);
                // Clean up temporary file
                let _ = std::fs::remove_file(&temp_path);
            }
        }
    } else {
        tracing::debug!("Cache disabled, skipping write");
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustowl::models::Function;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_mtime_validation_enabled_lru_invalidation() {
        let mut cache = CacheData::with_config(CacheConfig {
            validate_file_mtime: true,
            ..Default::default()
        });

        // Create a test function
        let test_function = Function::new(1);

        // Manually create a cache entry with a specific old mtime
        let old_mtime = 1; // Ensure it is older than real file mtime
        let entry = CacheEntry {
            function: test_function.clone(),
            created_at: old_mtime,
            last_accessed: old_mtime,
            access_count: 1,
            file_mtime: Some(old_mtime),
            data_size: 100,
        };

        let key = CacheData::make_key("test_file_hash", "test_mir_hash");
        cache.entries.insert(key, entry);

        // Create a temporary file with newer mtime
        let mut temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_string_lossy().to_string();
        writeln!(temp_file, "test content").unwrap();
        temp_file.flush().unwrap();

        // Verify cache miss due to modified file (cached mtime is older)
        let result = cache.get_cache("test_file_hash", "test_mir_hash", Some(&file_path));
        assert!(
            result.is_none(),
            "Cache should be invalidated when file mtime is newer than cached mtime"
        );
        let stats = cache.get_stats();
        assert_eq!(stats.invalidations, 1);
        assert_eq!(stats.evictions, 0, "Invalidation should not count as eviction");
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_mtime_validation_enabled_fifo_invalidation() {
        let mut cache = CacheData::with_config(CacheConfig {
            validate_file_mtime: true,
            use_lru_eviction: false,
            ..Default::default()
        });

        // Create a test function
        let test_function = Function::new(2);

        // Insert entry with old mtime
        let old_mtime = 1;
        let entry = CacheEntry {
            function: test_function,
            created_at: old_mtime,
            last_accessed: old_mtime,
            access_count: 1,
            file_mtime: Some(old_mtime),
            data_size: 64,
        };
        let key = CacheData::make_key("file_hash_fifo", "mir_hash_fifo");
        cache.entries.insert(key, entry);

        let mut temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_string_lossy().to_string();
        writeln!(temp_file, "fifo content").unwrap();
        temp_file.flush().unwrap();

        let result = cache.get_cache("file_hash_fifo", "mir_hash_fifo", Some(&file_path));
        assert!(result.is_none());
        let stats = cache.get_stats();
        assert_eq!(stats.invalidations, 1);
        assert_eq!(stats.evictions, 0);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.total_entries, 0, "Entry should be removed after invalidation");
    }

    #[test]
    fn test_mtime_validation_disabled() {
        let mut cache = CacheData::with_config(CacheConfig {
            validate_file_mtime: false,
            ..Default::default()
        });

        // Create a temporary file
        let mut temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_string_lossy().to_string();
        writeln!(temp_file, "test content").unwrap();

        // Create a test function
        let test_function = Function::new(3);

        // Insert cache entry
        cache.insert_cache_with_file_path(
            "test_file_hash".to_string(),
            "test_mir_hash".to_string(),
            test_function.clone(),
            Some(&file_path),
        );

        // Modify the file
        std::thread::sleep(std::time::Duration::from_millis(10));
        writeln!(temp_file, "modified content").unwrap();
        temp_file.flush().unwrap();

        // Verify cache hit even with modified file (validation disabled)
        let result = cache.get_cache("test_file_hash", "test_mir_hash", Some(&file_path));
        assert!(result.is_some());
        let stats = cache.get_stats();
        assert_eq!(stats.invalidations, 0);
        assert_eq!(stats.hits, 1);
    }

    #[test]
    fn test_mtime_validation_without_file_path() {
        let mut cache = CacheData::with_config(CacheConfig {
            validate_file_mtime: true,
            ..Default::default()
        });

        // Create a test function
        let test_function = Function::new(4);

        // Insert cache entry without file path
        cache.insert_cache_with_file_path(
            "test_file_hash".to_string(),
            "test_mir_hash".to_string(),
            test_function.clone(),
            None,
        );

        // Verify cache hit works without file path (no validation performed)
        let result = cache.get_cache("test_file_hash", "test_mir_hash", None);
        assert!(result.is_some());
        let stats = cache.get_stats();
        assert_eq!(stats.invalidations, 0);
        assert_eq!(stats.hits, 1);
    }

    #[test]
    fn test_mtime_validation_unchanged_hit() {
        let mut cache = CacheData::with_config(CacheConfig {
            validate_file_mtime: true,
            ..Default::default()
        });

        let mut temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_string_lossy().to_string();
        writeln!(temp_file, "initial content").unwrap();
        temp_file.flush().unwrap();

        let test_function = Function::new(5);
        cache.insert_cache_with_file_path(
            "unchanged_file_hash".to_string(),
            "unchanged_mir_hash".to_string(),
            test_function.clone(),
            Some(&file_path),
        );

        // No modification to the file -> should be a hit
        let result = cache.get_cache("unchanged_file_hash", "unchanged_mir_hash", Some(&file_path));
        assert!(result.is_some(), "Entry should remain valid when file unchanged");
        let stats = cache.get_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.invalidations, 0);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn test_get_file_mtime() {
        // Test with non-existent file
        assert!(CacheData::get_file_mtime("/non/existent/file").is_none());

        // Test with actual file
        let mut temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_string_lossy().to_string();
        writeln!(temp_file, "test content").unwrap();
        temp_file.flush().unwrap();

        let mtime = CacheData::get_file_mtime(&file_path);
        assert!(mtime.is_some());
        assert!(mtime.unwrap() > 0);
    }
}
