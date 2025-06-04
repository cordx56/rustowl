//! Benchmark target demonstrating feature and platform-specific performance code
//! This benchmark will have different code paths depending on compilation target

use rustowl_perf_test_dummy::arch_specific::SimdProcessor;

#[cfg(feature = "feature_b")]
use rustowl_perf_test_dummy::feature_b_module::CryptoHandler;

fn main() {
    println!("Running benchmarks");

    // Benchmark SIMD operations on different architectures
    let processor = SimdProcessor;
    let test_data: Vec<f32> = (0..1000).map(|i| i as f32).collect();

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _result = processor.process(&test_data);
    }
    let duration = start.elapsed();

    println!("SIMD processing took: {:?}", duration);

    // Architecture-specific benchmarks
    #[cfg(target_arch = "x86_64")]
    {
        println!("Running x86_64 specific benchmarks");
        let start = std::time::Instant::now();
        for _ in 0..10000 {
            let _result = processor.process_data_avx2(&test_data);
        }
        let duration = start.elapsed();
        println!("AVX2 benchmark: {:?}", duration);
    }

    #[cfg(target_arch = "aarch64")]
    {
        println!("Running ARM64 specific benchmarks");
        let start = std::time::Instant::now();
        for _ in 0..10000 {
            let _result = processor.process_data_neon(&test_data);
        }
        let duration = start.elapsed();
        println!("NEON benchmark: {:?}", duration);
    }

    // Feature-dependent benchmarks
    #[cfg(feature = "feature_b")]
    {
        println!("Running crypto benchmarks (feature_b enabled)");
        let crypto = CryptoHandler::new("benchmark_key".to_string());
        let test_bytes = b"benchmark data for encoding performance test";

        let start = std::time::Instant::now();
        for _ in 0..10000 {
            let encoded = crypto.encode_data(test_bytes);
            let _decoded = crypto.decode_data(&encoded).unwrap();
        }
        let duration = start.elapsed();
        println!("Crypto encode/decode benchmark: {:?}", duration);

        // Platform-specific entropy benchmarks
        #[cfg(unix)]
        {
            let start = std::time::Instant::now();
            for _ in 0..100 {
                let _entropy = crypto.get_system_entropy().unwrap();
            }
            let duration = start.elapsed();
            println!("Unix entropy benchmark: {:?}", duration);
        }

        #[cfg(windows)]
        {
            let start = std::time::Instant::now();
            for _ in 0..100 {
                let _entropy = crypto.get_system_entropy().unwrap();
            }
            let duration = start.elapsed();
            println!("Windows entropy benchmark: {:?}", duration);
        }
    }

    #[cfg(not(feature = "feature_b"))]
    {
        println!("Crypto benchmarks skipped (feature_b not enabled)");
    }

    // Platform-specific system operation benchmarks
    #[cfg(unix)]
    {
        use rustowl_perf_test_dummy::system_ops::unix_ops;

        println!("Running Unix-specific benchmarks");
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _info = unix_ops::get_current_process_info().unwrap();
        }
        let duration = start.elapsed();
        println!("Unix process info benchmark: {:?}", duration);
    }

    #[cfg(windows)]
    {
        use rustowl_perf_test_dummy::system_ops::windows_ops;

        println!("Running Windows-specific benchmarks");
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _info = windows_ops::get_current_process_info().unwrap();
        }
        let duration = start.elapsed();
        println!("Windows process info benchmark: {:?}", duration);

        #[cfg(feature = "feature_a")]
        {
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                let _thread_info = windows_ops::get_thread_info().unwrap();
            }
            let duration = start.elapsed();
            println!("Windows thread info benchmark: {:?}", duration);
        }
    }

    println!("Benchmarks completed");
}
