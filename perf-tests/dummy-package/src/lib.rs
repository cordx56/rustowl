//! RustOwl Performance Test Dummy Package
//! 
//! This is a dummy Rust package designed for performance testing with RustOwl.
//! It contains various Rust patterns and constructs that RustOwl can analyze,
//! including potential ownership issues, error handling patterns, and more.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};

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

impl FileManager {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn open_file(&self, path: &str) -> Result<()> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open file: {}", path))?;
        
        let mut files = self.files.lock().unwrap(); // Potential panic
        files.insert(path.to_string(), file);
        Ok(())
    }

    // Method with potential resource leak
    pub fn read_file_content(&self, path: &str) -> Result<String> {
        let files = self.files.lock().unwrap();
        let file = files.get(path).unwrap(); // Potential panic
        
        let mut content = String::new();
        // Note: This won't work because File doesn't implement Read when behind &
        // This is intentionally problematic code for testing
        // file.read_to_string(&mut content)?;
        Ok(content)
    }

    // Async operation that might have concurrency issues
    pub async fn process_files_async(&self) -> Result<Vec<String>> {
        let files = self.files.clone();
        
        let handle = tokio::spawn(async move {
            let files = files.lock().unwrap();
            files.keys().cloned().collect::<Vec<_>>()
        });

        handle.await.map_err(|e| anyhow::anyhow!("Task failed: {}", e))
    }
}

/// Network client with potential error handling issues
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    // Method with potential unwrap issues
    pub async fn fetch_data(&self, endpoint: &str) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.base_url, endpoint);
        let response = self.client.get(&url).send().await?;
        
        // Potential panic point
        let json = response.json::<serde_json::Value>().await.unwrap();
        Ok(json)
    }

    // Method with better error handling
    pub async fn fetch_data_safe(&self, endpoint: &str) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.base_url, endpoint);
        let response = self.client.get(&url).send().await
            .with_context(|| format!("Failed to send request to {}", url))?;
        
        let json = response.json::<serde_json::Value>().await
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
        
        let config: AppConfig = serde_json::from_str(&contents)
            .with_context(|| "Failed to parse configuration")?;
        
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
            container.metadata.insert("created_at".to_string(), chrono::Utc::now().to_string());
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
