use anyhow::Result;
use clap::{Arg, Command};
use log::{info, warn};
use rustowl_perf_test_dummy::*;

// Import platform-specific modules
#[cfg(feature = "feature_a")]
use rustowl_perf_test_dummy::feature_a_module::FeatureAHandler;

#[cfg(feature = "feature_b")]
use rustowl_perf_test_dummy::feature_b_module::CryptoHandler;

use rustowl_perf_test_dummy::arch_specific::SimdProcessor;
use rustowl_perf_test_dummy::system_ops;

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<()> {
    main_impl().await
}

#[cfg(not(feature = "tokio"))]
fn main() -> Result<()> {
    // For non-tokio builds, we can't run async operations
    env_logger::init();
    println!("Dummy app running without tokio support");
    println!("Limited functionality available");
    Ok(())
}

#[cfg(feature = "tokio")]
async fn main_impl() -> Result<()> {
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
                .value_parser([
                    "data", "network", "files", "compute", "platform", "features", "all",
                ]),
        )
        .arg(
            Arg::new("size")
                .short('s')
                .long("size")
                .help("Size parameter for operations")
                .value_name("SIZE")
                .default_value("100"),
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
        "platform" => run_platform_specific_tests().await?,
        "features" => run_feature_tests().await?,
        "all" => {
            run_data_operations(size).await?;
            run_network_operations().await?;
            run_file_operations(size)?;
            run_compute_operations(size)?;
            run_platform_specific_tests().await?;
            run_feature_tests().await?;
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

#[cfg(feature = "networking")]
async fn run_network_operations() -> Result<()> {
    info!("Running network operations");

    let client = ApiClient::new("https://httpbin.org".to_string());

    // Test network request (this might fail if no internet, which is fine for testing)
    match client.fetch_data_safe("get").await {
        Ok(data) => info!("Successfully fetched data: {:?}", data),
        Err(e) => warn!(
            "Network request failed (expected in some environments): {}",
            e
        ),
    }

    Ok(())
}

#[cfg(not(feature = "networking"))]
async fn run_network_operations() -> Result<()> {
    info!("Network operations skipped (networking feature not enabled)");
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
    info!(
        "Loaded config with {} endpoints",
        loaded_config.api_endpoints.len()
    );

    // Test potentially problematic method
    if let Some(endpoint) = loaded_config.get_first_endpoint_safe() {
        info!("First endpoint: {}", endpoint);
    }

    // Clean up
    std::fs::remove_file(&temp_config_path)?;

    Ok(())
}

async fn run_platform_specific_tests() -> Result<()> {
    info!("Running platform-specific tests");

    // Test cross-platform process info
    let process_info = system_ops::get_process_info()?;
    info!(
        "Process ID: {}, Name: {}",
        process_info.pid, process_info.name
    );

    // Test platform-specific features
    #[cfg(unix)]
    {
        info!("Running Unix-specific tests");
        #[cfg(feature = "feature_b")]
        {
            let crypto = CryptoHandler::new("unix_key".to_string());
            match crypto.get_system_entropy() {
                Ok(entropy) => info!("Got {} bytes of Unix entropy", entropy.len()),
                Err(e) => warn!("Failed to get Unix entropy: {}", e),
            }
        }

        if let Ok(perms) = system_ops::unix_ops::get_file_permissions("/tmp") {
            info!("Permissions for /tmp: {:o}", perms);
        }
    }

    #[cfg(windows)]
    {
        info!("Running Windows-specific tests");
        #[cfg(feature = "feature_a")]
        {
            let handler = FeatureAHandler::new();
            if let Err(e) = handler.show_message("Hello from Windows!") {
                warn!("Failed to show Windows message: {}", e);
            }

            if let Ok((pid, tid)) = system_ops::windows_ops::get_thread_info() {
                info!("Windows Process ID: {}, Thread ID: {}", pid, tid);
            }
        }

        #[cfg(feature = "feature_b")]
        {
            let crypto = CryptoHandler::new("windows_key".to_string());
            match crypto.get_system_entropy() {
                Ok(entropy) => info!("Got {} bytes of Windows entropy", entropy.len()),
                Err(e) => warn!("Failed to get Windows entropy: {}", e),
            }
        }

        if let Ok(value) =
            system_ops::windows_ops::read_registry_value("HKEY_LOCAL_MACHINE\\SOFTWARE\\Test")
        {
            info!("Registry value: {}", value);
        }
    }

    // Test architecture-specific code
    let simd = SimdProcessor;
    let test_data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let processed = simd.process(&test_data);
    info!("SIMD processed data: {:?}", processed);

    Ok(())
}

async fn run_feature_tests() -> Result<()> {
    info!("Running feature-specific tests");

    #[cfg(feature = "feature_a")]
    {
        info!("Feature A is enabled");
        let handler = FeatureAHandler::new();
        if let Err(e) = handler.show_message("Feature A test message") {
            log::error!("Feature A test failed: {}", e);
        }
    }

    #[cfg(not(feature = "feature_a"))]
    {
        info!("Feature A is disabled");
    }

    #[cfg(feature = "feature_b")]
    {
        info!("Feature B is enabled");
        let crypto = CryptoHandler::new("test_key_123".to_string());
        let test_data = b"Hello, world!";
        let encoded = crypto.encode_data(test_data);
        info!("Encoded data: {}", encoded);

        match crypto.decode_data(&encoded) {
            Ok(decoded) => {
                let decoded_str = String::from_utf8_lossy(&decoded);
                info!("Decoded data: {}", decoded_str);
            }
            Err(e) => log::error!("Failed to decode data: {}", e),
        }

        // Test platform-specific entropy
        match crypto.get_system_entropy() {
            Ok(entropy) => info!("System entropy length: {}", entropy.len()),
            Err(e) => log::warn!("Failed to get system entropy: {}", e),
        }
    }

    #[cfg(not(feature = "feature_b"))]
    {
        info!("Feature B is disabled");
    }

    #[cfg(feature = "networking")]
    {
        info!("Networking feature is enabled");
        // Would use reqwest here if networking feature is enabled
    }

    #[cfg(not(feature = "networking"))]
    {
        info!("Networking feature is disabled");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg_attr(not(miri), tokio::test)]
    #[cfg_attr(miri, test)]
    #[cfg_attr(miri, ignore)]
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
