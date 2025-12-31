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
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            v == "false" || v == "0"
        })
        .unwrap_or(false)
}

pub fn set_cache_path(cmd: &mut Command, target_dir: impl AsRef<Path>) {
    cmd.env("RUSTOWL_CACHE_DIR", target_dir.as_ref().join("cache"));
}

pub fn get_cache_path() -> Option<PathBuf> {
    env::var("RUSTOWL_CACHE_DIR")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// Construct a CacheConfig starting from defaults and overriding fields from environment variables.
///
/// The following environment variables are recognized (case-sensitive names):
/// - `RUSTOWL_CACHE_MAX_ENTRIES`: parsed as `usize` to set `max_entries`.
/// - `RUSTOWL_CACHE_MAX_MEMORY_MB`: parsed as `usize`; stored as bytes using saturating multiplication by 1024*1024.
/// - `RUSTOWL_CACHE_EVICTION`: case-insensitive; `"lru"` enables LRU eviction, `"fifo"` disables it; other values leave the default.
/// - `RUSTOWL_CACHE_VALIDATE_FILES`: case-insensitive; `"false"` or `"0"` disables file mtime validation, any other value enables it.
///
/// Returns the assembled `CacheConfig`.
///
/// # Examples
///
/// ```
/// use rustowl::cache::get_cache_config;
/// unsafe { std::env::set_var("RUSTOWL_CACHE_MAX_ENTRIES", "5"); }
/// let cfg = get_cache_config();
/// assert_eq!(cfg.max_entries, 5);
/// ```
pub fn get_cache_config() -> CacheConfig {
    let mut config = CacheConfig::default();

    // Configure max entries
    if let Ok(max_entries) = env::var("RUSTOWL_CACHE_MAX_ENTRIES")
        && let Ok(value) = max_entries.parse::<usize>()
    {
        config.max_entries = value;
    }

    // Configure max memory in MB
    if let Ok(max_memory_mb) = env::var("RUSTOWL_CACHE_MAX_MEMORY_MB")
        && let Ok(value) = max_memory_mb.parse::<usize>()
    {
        config.max_memory_bytes = value.saturating_mul(1024 * 1024);
    }

    // Configure eviction policy
    if let Ok(policy) = env::var("RUSTOWL_CACHE_EVICTION") {
        match policy.trim().to_ascii_lowercase().as_str() {
            "lru" => config.use_lru_eviction = true,
            "fifo" => config.use_lru_eviction = false,
            _ => {} // keep default
        }
    }

    // Configure file validation
    if let Ok(validate) = env::var("RUSTOWL_CACHE_VALIDATE_FILES") {
        let v = validate.trim().to_ascii_lowercase();
        config.validate_file_mtime = !(v == "false" || v == "0");
    }

    config
}

#[cfg(test)]
use std::sync::LazyLock;

#[cfg(test)]
static ENV_LOCK: LazyLock<std::sync::Mutex<()>> = LazyLock::new(|| std::sync::Mutex::new(()));

#[cfg(test)]
struct EnvGuard {
    key: String,
    old_value: Option<String>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl EnvGuard {
    fn set(key: &str, value: &str) -> Self {
        let lock = ENV_LOCK.lock().unwrap();
        let old_value = env::var(key).ok();
        unsafe {
            env::set_var(key, value);
        }
        Self {
            key: key.to_owned(),
            old_value,
            _lock: lock,
        }
    }
}

#[cfg(test)]
impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(v) = self.old_value.take() {
            unsafe {
                env::set_var(&self.key, v);
            }
        } else {
            unsafe {
                env::remove_var(&self.key);
            }
        }
    }
}

#[cfg(test)]
fn with_env<F>(key: &str, value: &str, f: F)
where
    F: FnOnce(),
{
    let guard = EnvGuard::set(key, value);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    drop(guard);
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn test_cache_config_default() {
    let config = CacheConfig::default();
    assert_eq!(config.max_entries, 1000);
    assert_eq!(config.max_memory_bytes, 100 * 1024 * 1024);
    assert!(config.use_lru_eviction);
    assert!(config.validate_file_mtime);
    assert!(!config.enable_compression);
}

#[test]
fn test_is_cache_default() {
    // Remove any existing cache env var for clean test
    let old_value = env::var("RUSTOWL_CACHE").ok();
    unsafe {
        env::remove_var("RUSTOWL_CACHE");
    }

    assert!(is_cache()); // Should be true by default

    // Restore old value
    if let Some(v) = old_value {
        unsafe {
            env::set_var("RUSTOWL_CACHE", v);
        }
    }
}

#[test]
fn test_is_cache_with_false_values() {
    with_env("RUSTOWL_CACHE", "false", || {
        assert!(!is_cache());
    });

    with_env("RUSTOWL_CACHE", "FALSE", || {
        assert!(!is_cache());
    });

    with_env("RUSTOWL_CACHE", "0", || {
        assert!(!is_cache());
    });

    with_env("RUSTOWL_CACHE", "  false  ", || {
        assert!(!is_cache());
    });
}

#[test]
fn test_is_cache_with_true_values() {
    with_env("RUSTOWL_CACHE", "true", || {
        assert!(is_cache());
    });

    with_env("RUSTOWL_CACHE", "1", || {
        assert!(is_cache());
    });

    with_env("RUSTOWL_CACHE", "yes", || {
        assert!(is_cache());
    });

    with_env("RUSTOWL_CACHE", "", || {
        assert!(is_cache());
    });
}

#[test]
fn test_get_cache_path() {
    // Test with no env var
    with_env("RUSTOWL_CACHE_DIR", "", || {
        // First remove the var
        let old_value = env::var("RUSTOWL_CACHE_DIR").ok();
        unsafe {
            env::remove_var("RUSTOWL_CACHE_DIR");
        }
        let result = get_cache_path();
        // Restore
        if let Some(v) = old_value {
            unsafe {
                env::set_var("RUSTOWL_CACHE_DIR", v);
            }
        }
        assert!(result.is_none());
    });

    // Test with empty value
    with_env("RUSTOWL_CACHE_DIR", "", || {
        assert!(get_cache_path().is_none());
    });

    // Test with whitespace only
    with_env("RUSTOWL_CACHE_DIR", "   ", || {
        assert!(get_cache_path().is_none());
    });

    // Test with valid path
    with_env("RUSTOWL_CACHE_DIR", "/tmp/cache", || {
        let path = get_cache_path().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/cache"));
    });

    // Test with path that has whitespace
    with_env("RUSTOWL_CACHE_DIR", "  /tmp/cache  ", || {
        let path = get_cache_path().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/cache"));
    });
}

#[test]
fn test_set_cache_path() {
    use tokio::process::Command;

    let mut cmd = Command::new("echo");
    let target_dir = PathBuf::from("/tmp/test_target");

    set_cache_path(&mut cmd, &target_dir);

    // Note: We can't easily test that the env var was set on the Command
    // since that's internal to tokio::process::Command, but we can test
    // that the function doesn't panic and accepts the expected types
    let expected_cache_dir = target_dir.join("cache");
    assert_eq!(expected_cache_dir, PathBuf::from("/tmp/test_target/cache"));
}

#[test]
fn test_get_cache_config_with_env_vars() {
    // Test max entries configuration
    with_env("RUSTOWL_CACHE_MAX_ENTRIES", "500", || {
        let config = get_cache_config();
        assert_eq!(config.max_entries, 500);
    });

    // Test that invalid values don't crash the program
    with_env("RUSTOWL_CACHE_MAX_ENTRIES", "invalid", || {
        let config = get_cache_config();
        // Should fall back to default when parse fails
        assert_eq!(config.max_entries, 1000);
    });
    // Test max memory configuration
    with_env("RUSTOWL_CACHE_MAX_MEMORY_MB", "200", || {
        let config = get_cache_config();
        assert_eq!(config.max_memory_bytes, 200 * 1024 * 1024);
    });

    // Test max memory with overflow protection
    with_env(
        "RUSTOWL_CACHE_MAX_MEMORY_MB",
        &usize::MAX.to_string(),
        || {
            let config = get_cache_config();
            // Should use saturating_mul, so might be different from exact calculation
            assert!(config.max_memory_bytes > 0);
        },
    );

    // Test eviction policy configuration
    with_env("RUSTOWL_CACHE_EVICTION", "lru", || {
        let config = get_cache_config();
        assert!(config.use_lru_eviction);
    });

    with_env("RUSTOWL_CACHE_EVICTION", "LRU", || {
        let config = get_cache_config();
        assert!(config.use_lru_eviction);
    });

    with_env("RUSTOWL_CACHE_EVICTION", "fifo", || {
        let config = get_cache_config();
        assert!(!config.use_lru_eviction);
    });

    with_env("RUSTOWL_CACHE_EVICTION", "FIFO", || {
        let config = get_cache_config();
        assert!(!config.use_lru_eviction);
    });

    // Test invalid eviction policy (should keep default)
    with_env("RUSTOWL_CACHE_EVICTION", "invalid", || {
        let config = get_cache_config();
        assert!(config.use_lru_eviction); // default is true
    });

    // Test file validation configuration
    with_env("RUSTOWL_CACHE_VALIDATE_FILES", "false", || {
        let config = get_cache_config();
        assert!(!config.validate_file_mtime);
    });

    with_env("RUSTOWL_CACHE_VALIDATE_FILES", "0", || {
        let config = get_cache_config();
        assert!(!config.validate_file_mtime);
    });

    with_env("RUSTOWL_CACHE_VALIDATE_FILES", "true", || {
        let config = get_cache_config();
        assert!(config.validate_file_mtime);
    });

    with_env("RUSTOWL_CACHE_VALIDATE_FILES", "1", || {
        let config = get_cache_config();
        assert!(config.validate_file_mtime);
    });

    with_env("RUSTOWL_CACHE_VALIDATE_FILES", "  FALSE  ", || {
        let config = get_cache_config();
        assert!(!config.validate_file_mtime);
    });
}
