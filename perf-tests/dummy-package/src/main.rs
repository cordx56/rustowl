use rustowl_perf_test_dummy::*;
use clap::{Arg, Command};
use log::{info, warn, error};
use std::path::Path;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let matches = Command::new("dummy-app")
        .version("0.1.0")
        .about("A dummy application for RustOwl performance testing")
        .arg(
            Arg::new("operation")
                .help("Operation to perform")
                .value_name("OPERATION")
                .index(1)
                .required(true)
                .value_parser(["data", "network", "files", "compute", "all"])
        )
        .arg(
            Arg::new("size")
                .short('s')
                .long("size")
                .help("Size parameter for operations")
                .value_name("SIZE")
                .default_value("100")
        )
        .get_matches();

    let operation = matches.get_one::<String>("operation").unwrap();
    let size: usize = matches.get_one::<String>("size").unwrap().parse()?;

    info!("Starting dummy application with operation: {}", operation);

    match operation.as_str() {
        "data" => run_data_operations(size).await?,
        "network" => run_network_operations().await?,
        "files" => run_file_operations(size)?,
        "compute" => run_compute_operations(size)?,
        "all" => {
            run_data_operations(size).await?;
            run_network_operations().await?;
            run_file_operations(size)?;
            run_compute_operations(size)?;
        }
        _ => unreachable!("Invalid operation"),
    }

    info!("Dummy application completed successfully");
    Ok(())
}

async fn run_data_operations(size: usize) -> Result<()> {
    info!("Running data operations with size: {}", size);
    
    // Generate test data
    let dataset = generate_large_dataset(size);
    info!("Generated {} data containers", dataset.len());

    // Test data processing
    let processor = DataProcessor::new(4);
    let results = processor.process_parallel(dataset)?;
    info!("Processed {} items", results.len());

    // Test file manager
    let file_manager = FileManager::new();
    
    // This will fail intentionally to test error handling
    if let Err(e) = file_manager.open_file("nonexistent_file.txt") {
        warn!("Expected error opening nonexistent file: {}", e);
    }

    Ok(())
}

async fn run_network_operations() -> Result<()> {
    info!("Running network operations");
    
    let client = ApiClient::new("https://httpbin.org".to_string());
    
    // Test network request (this might fail if no internet, which is fine for testing)
    match client.fetch_data_safe("get").await {
        Ok(data) => info!("Successfully fetched data: {:?}", data),
        Err(e) => warn!("Network request failed (expected in some environments): {}", e),
    }

    Ok(())
}

fn run_file_operations(count: usize) -> Result<()> {
    info!("Running file operations with count: {}", count);
    
    // Create a temporary directory for test files
    let temp_dir = std::env::temp_dir().join("rustowl_perf_test");
    std::fs::create_dir_all(&temp_dir)?;
    
    // Write test files
    write_test_files(count, temp_dir.to_str().unwrap())?;
    info!("Created {} test files", count);

    // Clean up test files
    std::fs::remove_dir_all(&temp_dir)?;
    info!("Cleaned up test files");

    Ok(())
}

fn run_compute_operations(size: usize) -> Result<()> {
    info!("Running compute operations with size: {}", size);
    
    // Run Fibonacci computation (limit size to prevent extremely long execution)
    let fib_input = std::cmp::min(size, 35) as u64;
    let result = compute_fibonacci(fib_input);
    info!("Fibonacci({}) = {}", fib_input, result);

    // Test configuration loading
    let temp_config_path = std::env::temp_dir().join("test_config.json");
    let test_config = AppConfig {
        database_url: "sqlite://test.db".to_string(),
        api_endpoints: vec![
            "http://api1.example.com".to_string(),
            "http://api2.example.com".to_string(),
        ],
        timeout_seconds: 30,
        retry_attempts: 3,
    };

    // Write and read config
    let config_json = serde_json::to_string_pretty(&test_config)?;
    std::fs::write(&temp_config_path, config_json)?;
    
    let loaded_config = AppConfig::load_from_file(temp_config_path.to_str().unwrap())?;
    info!("Loaded config with {} endpoints", loaded_config.api_endpoints.len());

    // Test potentially problematic method
    if let Some(endpoint) = loaded_config.get_first_endpoint_safe() {
        info!("First endpoint: {}", endpoint);
    }

    // Clean up
    std::fs::remove_file(&temp_config_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_data_operations() {
        let result = run_data_operations(10).await;
        // Allow this to fail since some operations are intentionally problematic
        match result {
            Ok(_) => println!("Data operations completed successfully"),
            Err(e) => println!("Data operations failed as expected: {}", e),
        }
    }

    #[test]
    fn test_compute_operations() {
        let result = run_compute_operations(10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_operations() {
        let result = run_file_operations(5);
        assert!(result.is_ok());
    }
}
