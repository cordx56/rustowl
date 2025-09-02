use std::env;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Configuration for cache behavior
#[derive(Clone, Debug)]
pub struct CacheConfig {
    /// Maximum number of entries before eviction
    pub max_entries: usize,
    /// Maximum memory usage in bytes before eviction
    pub max_memory_bytes: usize,
    /// Enable LRU eviction policy (vs FIFO)
    pub use_lru_eviction: bool,
    /// Enable file modification time validation
    pub validate_file_mtime: bool,
    /// Enable compression for cache files
    pub enable_compression: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            use_lru_eviction: true,
            validate_file_mtime: true,
            enable_compression: false, // Disable by default for compatibility
        }
    }
}

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

/// Get cache configuration from environment variables
pub fn get_cache_config() -> CacheConfig {
    let mut config = CacheConfig::default();
    
    // Configure max entries
    if let Ok(max_entries) = env::var("RUSTOWL_CACHE_MAX_ENTRIES") {
        if let Ok(value) = max_entries.parse::<usize>() {
            config.max_entries = value;
        }
    }
    
    // Configure max memory in MB
    if let Ok(max_memory_mb) = env::var("RUSTOWL_CACHE_MAX_MEMORY_MB") {
        if let Ok(value) = max_memory_mb.parse::<usize>() {
            config.max_memory_bytes = value * 1024 * 1024;
        }
    }
    
    // Configure eviction policy
    if let Ok(policy) = env::var("RUSTOWL_CACHE_EVICTION") {
        config.use_lru_eviction = policy.to_lowercase() == "lru";
    }
    
    // Configure file validation
    if let Ok(validate) = env::var("RUSTOWL_CACHE_VALIDATE_FILES") {
        config.validate_file_mtime = validate != "false" && validate != "0";
    }
    
    config
}
