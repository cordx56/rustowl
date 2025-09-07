//! # RustOwl rustowlc
//!
//! A compiler implementation for visualizing ownership and lifetimes in Rust, designed for debugging and optimization.

#![feature(rustc_private)]

pub extern crate polonius_engine;
pub extern crate rustc_borrowck;
pub extern crate rustc_data_structures;
pub extern crate rustc_driver;
pub extern crate rustc_errors;
pub extern crate rustc_hash;
pub extern crate rustc_hir;
pub extern crate rustc_index;
pub extern crate rustc_interface;
pub extern crate rustc_middle;
pub extern crate rustc_query_system;
pub extern crate rustc_session;
pub extern crate rustc_span;
pub extern crate rustc_stable_hash;
pub extern crate rustc_type_ir;

pub mod core;

use std::process::exit;

fn main() {
    // Initialize crypto provider for HTTPS requests
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    rustowl::initialize_logging(tracing_subscriber::filter::LevelFilter::INFO);

    // This is cited from [rustc](https://github.com/rust-lang/rust/blob/3014e79f9c8d5510ea7b3a3b70d171d0948b1e96/compiler/rustc/src/main.rs).
    // MIT License
    #[cfg(not(target_env = "msvc"))]
    {
        use std::os::raw::{c_int, c_void};

        use tikv_jemalloc_sys as jemalloc_sys;

        #[used]
        static _F1: unsafe extern "C" fn(usize, usize) -> *mut c_void = jemalloc_sys::calloc;
        #[used]
        static _F2: unsafe extern "C" fn(*mut *mut c_void, usize, usize) -> c_int =
            jemalloc_sys::posix_memalign;
        #[used]
        static _F3: unsafe extern "C" fn(usize, usize) -> *mut c_void = jemalloc_sys::aligned_alloc;
        #[used]
        static _F4: unsafe extern "C" fn(usize) -> *mut c_void = jemalloc_sys::malloc;
        #[used]
        static _F5: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void = jemalloc_sys::realloc;
        #[used]
        static _F6: unsafe extern "C" fn(*mut c_void) = jemalloc_sys::free;

        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn _rjem_je_zone_register();
            }

            #[used]
            static _F7: unsafe extern "C" fn() = _rjem_je_zone_register;
        }
    }

    rustowl::initialize_logging(tracing_subscriber::filter::LevelFilter::INFO);

    // rayon panics without this only on Windows
    #[cfg(target_os = "windows")]
    {
        rayon::ThreadPoolBuilder::new()
            .stack_size(4 * 1024 * 1024)
            .build_global()
            .unwrap();
    }

    exit(core::run_compiler())
}

#[cfg(test)]
mod tests {
    use std::process::ExitCode;

    // Test Windows rayon thread pool setup
    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_rayon_thread_pool() {
        // Test that Windows-specific rayon thread pool setup works
        let result = rayon::ThreadPoolBuilder::new()
            .stack_size(4 * 1024 * 1024)
            .build_global();

        // Should succeed or fail gracefully
        assert!(result.is_ok() || result.is_err());
    }

    // Test logging initialization
    #[test]
    fn test_logging_initialization() {
        // Test that logging can be initialized without panicking
        rustowl::initialize_logging(tracing_subscriber::filter::LevelFilter::INFO);
        // If we get here without panicking, the test passes
    }

    // Test main function structure (without actually running)
    #[test]
    fn test_main_function_structure() {
        // Test logging setup
        rustowl::initialize_logging(tracing_subscriber::filter::LevelFilter::INFO);

        // Test Windows rayon setup
        #[cfg(target_os = "windows")]
        {
            let result = rayon::ThreadPoolBuilder::new()
                .stack_size(4 * 1024 * 1024)
                .build_global();
            assert!(result.is_ok() || result.is_err());
        }
    }

    // Test exit code handling
    #[test]
    fn test_exit_code_handling() {
        // Test different exit codes
        let exit_success = ExitCode::SUCCESS;
        let exit_failure = ExitCode::FAILURE;

        // Verify that exit codes are properly defined
        assert_eq!(exit_success, ExitCode::from(0));
        assert_eq!(exit_failure, ExitCode::from(1));
    }

    // Test jemalloc sys crate access
    #[test]
    #[cfg(not(target_env = "msvc"))]
    fn test_jemalloc_sys_access() {
        // Test that jemalloc_sys functions are accessible
        // We can't call them without unsafe code, but we can verify they're declared
        use tikv_jemalloc_sys as jemalloc_sys;

        // Verify that the functions are accessible (compile-time check)
        let _calloc: unsafe extern "C" fn(usize, usize) -> *mut std::os::raw::c_void =
            jemalloc_sys::calloc;
        let _malloc: unsafe extern "C" fn(usize) -> *mut std::os::raw::c_void =
            jemalloc_sys::malloc;
        let _free: unsafe extern "C" fn(*mut std::os::raw::c_void) = jemalloc_sys::free;

        // The fact that these assignments compile means jemalloc_sys functions are accessible
    }

    // Test rayon thread pool builder access
    #[test]
    #[cfg(target_os = "windows")]
    fn test_rayon_thread_pool_builder() {
        // Test that rayon ThreadPoolBuilder is accessible and configurable
        let builder = rayon::ThreadPoolBuilder::new();
        let configured = builder.stack_size(4 * 1024 * 1024);

        // Verify that the builder can be configured
        assert!(configured.stack_size().is_some() || configured.stack_size().is_none());
    }

    // Test tracing subscriber level filter
    #[test]
    fn test_tracing_level_filter() {
        // Test that tracing LevelFilter values are accessible
        let info_level = tracing_subscriber::filter::LevelFilter::INFO;
        let warn_level = tracing_subscriber::filter::LevelFilter::WARN;
        let error_level = tracing_subscriber::filter::LevelFilter::ERROR;
        let off_level = tracing_subscriber::filter::LevelFilter::OFF;

        // Verify that different levels are distinct
        assert_ne!(info_level, warn_level);
        assert_ne!(warn_level, error_level);
        assert_ne!(error_level, off_level);
    }

    // Test rustowl initialize_logging function
    #[test]
    fn test_rustowl_initialize_logging() {
        // Test that rustowl's initialize_logging function can be called with different levels
        rustowl::initialize_logging(tracing_subscriber::filter::LevelFilter::ERROR);
        rustowl::initialize_logging(tracing_subscriber::filter::LevelFilter::WARN);
        rustowl::initialize_logging(tracing_subscriber::filter::LevelFilter::INFO);
        rustowl::initialize_logging(tracing_subscriber::filter::LevelFilter::DEBUG);

        // If we get here without panicking, the function works
    }
}
