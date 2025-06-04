//! RustOwl Performance Test Dummy Package
//!
//! This is a dummy Rust package designed for performance testing with RustOwl.
//! It contains various Rust patterns and constructs that RustOwl can analyze,
//! including potential ownership issues, error handling patterns, and more.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

/// A data structure that might have ownership issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataContainer {
    pub id: String,
    pub data: Vec<u8>,
    pub metadata: HashMap<String, String>,
}

impl DataContainer {
    pub fn new(id: String) -> Self {
        Self {
            id,
            data: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_data(&mut self, data: Vec<u8>) -> Result<()> {
        self.data.extend(data);
        Ok(())
    }

    // Potential ownership issue: returning reference to internal data
    pub fn get_data(&self) -> &[u8] {
        &self.data
    }

    // Method that could cause issues with unwrap()
    pub fn get_metadata(&self, key: &str) -> String {
        self.metadata.get(key).unwrap().clone() // Potential panic
    }

    // Better error handling version
    pub fn get_metadata_safe(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// File operations that might have resource management issues
pub struct FileManager {
    files: Arc<Mutex<HashMap<String, File>>>,
}

impl Default for FileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FileManager {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn open_file(&self, path: &str) -> Result<()> {
        let file = File::open(path).with_context(|| format!("Failed to open file: {}", path))?;

        let mut files = self.files.lock().unwrap(); // Potential panic
        files.insert(path.to_string(), file);
        Ok(())
    }

    // Method with potential resource leak
    pub fn read_file_content(&self, path: &str) -> Result<String> {
        let files = self.files.lock().unwrap();
        let _ = files.get(path).unwrap(); // Potential panic

        let content = String::new();
        // Note: This won't work because File doesn't implement Read when behind &
        // This is intentionally problematic code for testing
        // file.read_to_string(&mut content)?;
        Ok(content)
    }

    // Async operation that might have concurrency issues
    #[cfg(feature = "tokio")]
    pub async fn process_files_async(&self) -> Result<Vec<String>> {
        let files = self.files.clone();

        let handle = tokio::spawn(async move {
            let files = files.lock().unwrap();
            files.keys().cloned().collect::<Vec<_>>()
        });

        handle
            .await
            .map_err(|e| anyhow::anyhow!("Task failed: {}", e))
    }
}

/// Network client with potential error handling issues
#[cfg(feature = "networking")]
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

#[cfg(feature = "networking")]
impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    // Method with potential unwrap issues
    #[cfg(feature = "networking")]
    pub async fn fetch_data(&self, endpoint: &str) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.base_url, endpoint);
        let response = self.client.get(&url).send().await?;

        // Potential panic point
        let json = response.json::<serde_json::Value>().await.unwrap();
        Ok(json)
    }

    // Method with better error handling
    #[cfg(feature = "networking")]
    pub async fn fetch_data_safe(&self, endpoint: &str) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.base_url, endpoint);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?;

        let json = response
            .json::<serde_json::Value>()
            .await
            .with_context(|| "Failed to parse JSON response")?;
        Ok(json)
    }
}

/// Thread-based processor with potential concurrency issues
pub struct DataProcessor {
    workers: usize,
}

impl DataProcessor {
    pub fn new(workers: usize) -> Self {
        Self { workers }
    }

    // Method that might have thread safety issues
    pub fn process_parallel(&self, data: Vec<DataContainer>) -> Result<Vec<String>> {
        let shared_results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = vec![];

        for chunk in data.chunks(self.workers) {
            let results = shared_results.clone();
            let chunk = chunk.to_vec();

            let handle = thread::spawn(move || {
                for item in chunk {
                    let processed = format!("Processed: {}", item.id);
                    let mut results = results.lock().unwrap(); // Potential deadlock
                    results.push(processed);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap(); // Potential panic
        }

        let results = shared_results.lock().unwrap();
        Ok(results.clone())
    }
}

/// Configuration struct with potential ownership patterns
#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub api_endpoints: Vec<String>,
    pub timeout_seconds: u64,
    pub retry_attempts: usize,
}

impl AppConfig {
    pub fn load_from_file(path: &str) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config: AppConfig =
            serde_json::from_str(&contents).with_context(|| "Failed to parse configuration")?;

        Ok(config)
    }

    // Method that returns owned data that could be optimized
    pub fn get_first_endpoint(&self) -> String {
        self.api_endpoints.first().unwrap().clone() // Potential panic + unnecessary clone
    }

    // Better version
    pub fn get_first_endpoint_safe(&self) -> Option<&str> {
        self.api_endpoints.first().map(|s| s.as_str())
    }
}

/// Memory-intensive operations for performance testing
pub fn generate_large_dataset(size: usize) -> Vec<DataContainer> {
    (0..size)
        .map(|i| {
            let mut container = DataContainer::new(format!("item_{}", i));
            let data = vec![i as u8; 1024]; // 1KB per item
            container.add_data(data).unwrap(); // Potential panic
            container
                .metadata
                .insert("created_at".to_string(), chrono::Utc::now().to_string());
            container
        })
        .collect()
}

/// Complex computation for CPU benchmarking
pub fn compute_fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => compute_fibonacci(n - 1) + compute_fibonacci(n - 2), // Inefficient recursion
    }
}

/// IO-intensive operations
pub fn write_test_files(count: usize, base_path: &str) -> Result<()> {
    for i in 0..count {
        let filename = format!("{}/test_file_{}.txt", base_path, i);
        let mut file = File::create(&filename)
            .with_context(|| format!("Failed to create file: {}", filename))?;

        let content = format!("Test content for file {}\n{}", i, "x".repeat(1024));
        file.write_all(content.as_bytes())
            .with_context(|| format!("Failed to write to file: {}", filename))?;
    }

    Ok(())
}

// Feature-gated modules for testing --all-features flag
#[cfg(feature = "feature_a")]
pub mod feature_a_module {
    //! Feature A functionality - only available when feature_a is enabled

    #[cfg(windows)]
    use winapi::um::winuser::{MB_OK, MessageBoxW};

    pub struct FeatureAHandler {
        pub enabled: bool,
    }

    impl FeatureAHandler {
        pub fn new() -> Self {
            Self { enabled: true }
        }

        #[cfg(windows)]
        pub fn show_message(&self, message: &str) -> anyhow::Result<()> {
            // Windows-specific code using winapi
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;

            let wide_message: Vec<u16> = OsStr::new(message).encode_wide().chain(Some(0)).collect();
            let wide_title: Vec<u16> = OsStr::new("Feature A")
                .encode_wide()
                .chain(Some(0))
                .collect();

            unsafe {
                MessageBoxW(
                    std::ptr::null_mut(),
                    wide_message.as_ptr(),
                    wide_title.as_ptr(),
                    MB_OK,
                );
            }
            Ok(())
        }

        #[cfg(not(windows))]
        pub fn show_message(&self, message: &str) -> anyhow::Result<()> {
            println!("Feature A message: {}", message);
            Ok(())
        }
    }
}

#[cfg(feature = "feature_b")]
pub mod feature_b_module {
    //! Feature B functionality - only available when feature_b is enabled

    use anyhow::Result;
    use base64::{Engine as _, engine::general_purpose};

    pub struct CryptoHandler {
        pub key: String,
    }

    impl CryptoHandler {
        pub fn new(key: String) -> Self {
            Self { key }
        }

        pub fn encode_data(&self, data: &[u8]) -> String {
            general_purpose::STANDARD.encode(data)
        }

        pub fn decode_data(&self, encoded: &str) -> Result<Vec<u8>> {
            general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| anyhow::anyhow!("Base64 decode error: {}", e))
        }

        // Platform-specific cryptographic operations
        #[cfg(unix)]
        pub fn get_system_entropy(&self) -> Result<Vec<u8>> {
            use std::fs::File;
            use std::io::Read;

            let mut file = File::open("/dev/urandom")?;
            let mut buffer = vec![0u8; 32];
            file.read_exact(&mut buffer)?;
            Ok(buffer)
        }

        #[cfg(windows)]
        pub fn get_system_entropy(&self) -> Result<Vec<u8>> {
            // Simulate Windows crypto API usage
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            use std::time::SystemTime;

            let mut hasher = DefaultHasher::new();
            SystemTime::now().hash(&mut hasher);
            self.key.hash(&mut hasher);

            let hash = hasher.finish();
            Ok(hash.to_le_bytes().to_vec())
        }

        #[cfg(target_family = "wasm")]
        pub fn get_system_entropy(&self) -> Result<Vec<u8>> {
            // WASM-specific implementation
            Ok(vec![42u8; 32]) // Dummy entropy for WASM
        }
    }
}

// Platform-specific system operations module
pub mod system_ops {
    use anyhow::Result;

    /// Platform-specific process information
    pub struct ProcessInfo {
        pub pid: u32,
        pub name: String,
    }

    #[cfg(unix)]
    pub mod unix_ops {
        use super::*;
        use libc::{getpid, getppid};

        pub fn get_current_process_info() -> Result<ProcessInfo> {
            let pid = unsafe { getpid() } as u32;
            let ppid = unsafe { getppid() } as u32;

            Ok(ProcessInfo {
                pid,
                name: format!("unix_process_{}", ppid),
            })
        }

        pub fn set_process_priority(priority: i32) -> Result<()> {
            unsafe {
                if libc::nice(priority) == -1 {
                    return Err(anyhow::anyhow!("Failed to set process priority"));
                }
            }
            Ok(())
        }

        // Unix-specific file operations
        pub fn get_file_permissions(path: &str) -> Result<u32> {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(path)?;
            Ok(metadata.permissions().mode())
        }
    }

    #[cfg(windows)]
    pub mod windows_ops {
        use super::*;

        #[cfg(feature = "feature_a")]
        use winapi::um::processthreadsapi::{GetCurrentProcessId, GetCurrentThreadId};

        pub fn get_current_process_info() -> Result<ProcessInfo> {
            #[cfg(feature = "feature_a")]
            let pid = unsafe { GetCurrentProcessId() };

            #[cfg(not(feature = "feature_a"))]
            let pid = std::process::id();

            Ok(ProcessInfo {
                pid,
                name: format!("windows_process_{}", pid),
            })
        }

        #[cfg(feature = "feature_a")]
        pub fn get_thread_info() -> Result<(u32, u32)> {
            let process_id = unsafe { GetCurrentProcessId() };
            let thread_id = unsafe { GetCurrentThreadId() };
            Ok((process_id, thread_id))
        }

        #[cfg(not(feature = "feature_a"))]
        pub fn get_thread_info() -> Result<(u32, u32)> {
            let process_id = std::process::id();
            Ok((process_id, 0)) // Thread ID not available without winapi
        }

        // Windows-specific registry operations (simulation)
        pub fn read_registry_value(key: &str) -> Result<String> {
            // Simulate registry access
            match key {
                "HKEY_LOCAL_MACHINE\\SOFTWARE\\Test" => Ok("test_value".to_string()),
                _ => Err(anyhow::anyhow!("Registry key not found: {}", key)),
            }
        }
    }

    // Cross-platform interface
    pub fn get_process_info() -> Result<ProcessInfo> {
        #[cfg(unix)]
        return unix_ops::get_current_process_info();

        #[cfg(windows)]
        return windows_ops::get_current_process_info();

        #[cfg(not(any(unix, windows)))]
        Ok(ProcessInfo {
            pid: std::process::id(),
            name: "unknown_platform".to_string(),
        })
    }
}

// Architecture-specific optimizations
pub mod arch_specific {
    /// SIMD operations available on different architectures
    pub struct SimdProcessor;

    impl SimdProcessor {
        #[cfg(target_arch = "x86_64")]
        pub fn process_data_avx2(&self, data: &[f32]) -> Vec<f32> {
            // Simulate AVX2 SIMD operations
            data.iter().map(|x| x * 2.0).collect()
        }

        #[cfg(target_arch = "aarch64")]
        pub fn process_data_neon(&self, data: &[f32]) -> Vec<f32> {
            // Simulate ARM NEON SIMD operations
            data.iter().map(|x| x * 1.5).collect()
        }

        #[cfg(target_arch = "wasm32")]
        pub fn process_data_wasm_simd(&self, data: &[f32]) -> Vec<f32> {
            // WASM SIMD operations
            data.iter().map(|x| x + 1.0).collect()
        }

        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "wasm32"
        )))]
        pub fn process_data_generic(&self, data: &[f32]) -> Vec<f32> {
            // Fallback implementation
            data.to_vec()
        }

        pub fn process(&self, data: &[f32]) -> Vec<f32> {
            #[cfg(target_arch = "x86_64")]
            return self.process_data_avx2(data);

            #[cfg(target_arch = "aarch64")]
            return self.process_data_neon(data);

            #[cfg(target_arch = "wasm32")]
            return self.process_data_wasm_simd(data);

            #[cfg(not(any(
                target_arch = "x86_64",
                target_arch = "aarch64",
                target_arch = "wasm32"
            )))]
            return self.process_data_generic(data);
        }
    }
}

// Testing different target environments
#[cfg(test)]
mod target_tests {
    use super::*;

    #[test]
    fn test_cross_platform_process_info() {
        let info = system_ops::get_process_info().unwrap();
        assert!(info.pid > 0);
        assert!(!info.name.is_empty());
    }

    #[cfg(feature = "feature_a")]
    #[test]
    fn test_feature_a() {
        let handler = feature_a_module::FeatureAHandler::new();
        assert!(handler.enabled);
    }

    #[cfg(feature = "feature_b")]
    #[test]
    fn test_feature_b() {
        let handler = feature_b_module::CryptoHandler::new("test_key".to_string());
        let data = b"test data";
        let encoded = handler.encode_data(data);
        let decoded = handler.decode_data(&encoded).unwrap();
        assert_eq!(data, decoded.as_slice());
    }

    #[cfg(all(feature = "feature_b", unix))]
    #[test]
    fn test_unix_entropy() {
        let handler = feature_b_module::CryptoHandler::new("test".to_string());
        let entropy = handler.get_system_entropy().unwrap();
        assert_eq!(entropy.len(), 32);
    }

    #[cfg(all(feature = "feature_b", windows))]
    #[test]
    fn test_windows_entropy() {
        let handler = feature_b_module::CryptoHandler::new("test".to_string());
        let entropy = handler.get_system_entropy().unwrap();
        assert_eq!(entropy.len(), 8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_container() {
        let mut container = DataContainer::new("test".to_string());
        container.add_data(vec![1, 2, 3]).unwrap();
        assert_eq!(container.get_data(), &[1, 2, 3]);
    }

    #[test]
    #[should_panic]
    fn test_metadata_panic() {
        let container = DataContainer::new("test".to_string());
        container.get_metadata("nonexistent"); // This should panic
    }

    #[test]
    fn test_fibonacci() {
        assert_eq!(compute_fibonacci(5), 5);
        assert_eq!(compute_fibonacci(10), 55);
    }
}
