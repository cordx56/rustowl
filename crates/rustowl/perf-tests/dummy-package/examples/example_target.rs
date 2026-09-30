//! Example demonstrating target-specific compilation
//! This example will compile differently on different platforms

use rustowl_perf_test_dummy::system_ops;


#[cfg(feature = "feature_b")]
use rustowl_perf_test_dummy::feature_b_module::CryptoHandler;

fn main() -> anyhow::Result<()> {
    println!("Running platform-specific example");

    // Test process info (cross-platform)
    let process_info = system_ops::get_process_info()?;
    println!("Process: {} (PID: {})", process_info.name, process_info.pid);

    // Platform-specific operations
    #[cfg(unix)]
    {
        println!("This is a Unix system!");
        #[cfg(feature = "feature_b")]
        {
            let crypto = CryptoHandler::new("example_key".to_string());
            match crypto.get_system_entropy() {
                Ok(entropy) => println!("Got {} bytes of Unix entropy", entropy.len()),
                Err(e) => eprintln!("Failed to get entropy: {}", e),
            }
        }

        // Try to access Unix-specific file permissions
        match system_ops::unix_ops::get_file_permissions("/etc") {
            Ok(perms) => println!("Permissions for /etc: {:o}", perms),
            Err(e) => eprintln!("Failed to get permissions: {}", e),
        }
    }

    #[cfg(windows)]
    {
        println!("This is a Windows system!");

        #[cfg(feature = "feature_a")]
        {
            let handler = FeatureAHandler::new();
            if let Err(e) = handler.show_message("Hello from example!") {
                eprintln!("Failed to show message: {}", e);
            }

            match system_ops::windows_ops::get_thread_info() {
                Ok((pid, tid)) => println!("Process ID: {}, Thread ID: {}", pid, tid),
                Err(e) => eprintln!("Failed to get thread info: {}", e),
            }
        }

        #[cfg(feature = "feature_b")]
        {
            let crypto = CryptoHandler::new("windows_example_key".to_string());
            match crypto.get_system_entropy() {
                Ok(entropy) => println!("Got {} bytes of Windows entropy", entropy.len()),
                Err(e) => eprintln!("Failed to get entropy: {}", e),
            }
        }

        // Try to read a registry value
        match system_ops::windows_ops::read_registry_value("HKEY_LOCAL_MACHINE\\SOFTWARE\\Test") {
            Ok(value) => println!("Registry value: {}", value),
            Err(e) => eprintln!("Failed to read registry: {}", e),
        }
    }

    // Architecture-specific code
    #[cfg(target_arch = "x86_64")]
    {
        println!("Running on x86_64 architecture");
        let simd = rustowl_perf_test_dummy::arch_specific::SimdProcessor;
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let processed = simd.process(&data);
        println!("AVX2 processed: {:?}", processed);
    }

    #[cfg(target_arch = "aarch64")]
    {
        println!("Running on ARM64 architecture");
        let simd = rustowl_perf_test_dummy::arch_specific::SimdProcessor;
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let processed = simd.process(&data);
        println!("NEON processed: {:?}", processed);
    }

    #[cfg(target_arch = "wasm32")]
    {
        println!("Running on WebAssembly");
        let simd = rustowl_perf_test_dummy::arch_specific::SimdProcessor;
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let processed = simd.process(&data);
        println!("WASM SIMD processed: {:?}", processed);
    }

    // Target environment specific code
    #[cfg(target_env = "msvc")]
    {
        println!("Compiled with MSVC toolchain");
    }

    #[cfg(target_env = "gnu")]
    {
        println!("Compiled with GNU toolchain");
    }

    #[cfg(target_family = "unix")]
    {
        println!("Target family: Unix");
    }

    #[cfg(target_family = "windows")]
    {
        println!("Target family: Windows");
    }

    // Feature-specific code that will only compile when features are enabled
    #[cfg(all(feature = "feature_a", feature = "feature_b"))]
    {
        println!("Both Feature A and Feature B are enabled!");
    }

    #[cfg(feature = "networking")]
    {
        println!("Networking feature is enabled");
        // Could make HTTP requests here if needed
    }

    #[cfg(feature = "advanced_crypto")]
    {
        println!("Advanced crypto feature is enabled (depends on feature_b)");
    }

    Ok(())
}
