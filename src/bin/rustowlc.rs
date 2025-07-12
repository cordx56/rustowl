//! # RustOwl rustowlc
//!
//! A compiler implementation for visualizing ownership and lifetimes in Rust, designed for debugging and optimization.

#![feature(rustc_private)]

pub extern crate indexmap;
pub extern crate polonius_engine;
pub extern crate rustc_borrowck;
pub extern crate rustc_driver;
pub extern crate rustc_errors;
pub extern crate rustc_hash;
pub extern crate rustc_hir;
pub extern crate rustc_index;
pub extern crate rustc_interface;
pub extern crate rustc_middle;
pub extern crate rustc_session;
pub extern crate rustc_span;
pub extern crate smallvec;

pub mod core;

use std::process::exit;

// Use static linking for macOS ARM for better symbol resolution
#[cfg(enable_static_link)]
use std::sync::atomic::AtomicBool;

#[cfg(enable_static_link)]
#[allow(dead_code)]
static STATIC_LINK_ENABLED: AtomicBool = AtomicBool::new(true);

fn main() {
    // This is cited from [rustc](https://github.com/rust-lang/rust/blob/b90cfc887c31c3e7a9e6d462e2464db1fe506175/compiler/rustc/src/main.rs).
    // MIT License
    {
        use std::os::raw::{c_int, c_void};

        #[used]
        static _F1: unsafe extern "C" fn(usize, usize) -> *mut c_void = libmimalloc_sys::mi_calloc;
        #[used]
        static _F2: unsafe extern "C" fn(*mut *mut c_void, usize, usize) -> c_int =
            libmimalloc_sys::mi_posix_memalign;
        #[used]
        static _F3: unsafe extern "C" fn(usize, usize) -> *mut c_void =
            libmimalloc_sys::mi_aligned_alloc;
        #[used]
        static _F4: unsafe extern "C" fn(usize) -> *mut c_void = libmimalloc_sys::mi_malloc;
        #[used]
        static _F5: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void =
            libmimalloc_sys::mi_realloc;
        #[used]
        static _F6: unsafe extern "C" fn(*mut c_void) = libmimalloc_sys::mi_free;

        // Only use _mi_macros_override_malloc on non-ARM64 macOS platforms
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            unsafe extern "C" {
                fn _mi_macros_override_malloc();
            }

            #[used]
            static _F7: unsafe extern "C" fn() = _mi_macros_override_malloc;
        }

        // For macOS ARM, add additional allocator symbol exports
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            // Export additional symbols needed for macOS ARM compatibility
            #[used]
            static _F8: unsafe extern "C" fn(usize, usize, usize) -> *mut c_void =
                libmimalloc_sys::mi_calloc_aligned;
            #[used]
            static _F9: unsafe extern "C" fn(usize, usize) -> *mut c_void =
                libmimalloc_sys::mi_malloc_aligned;
        }

        // Set up macOS ARM environment variables for dynamic library loading
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            // Set DYLD_FALLBACK_LIBRARY_PATH for macOS if not already set
            if std::env::var_os("DYLD_FALLBACK_LIBRARY_PATH").is_none() {
                let sysroot = rustowl::toolchain::get_sysroot_sync();

                // Find the actual library directory containing rustc_driver
                if let Some(driver_path) = rustowl::toolchain::rustc_driver_path(&sysroot) {
                    if let Some(lib_dir) = driver_path.parent() {
                        let lib_path = format!("{}:/usr/local/lib:/usr/lib", lib_dir.display());
                        unsafe {
                            std::env::set_var("DYLD_FALLBACK_LIBRARY_PATH", &lib_path);
                        }
                        eprintln!("macOS ARM detected: Set DYLD_FALLBACK_LIBRARY_PATH={lib_path}");
                    }
                } else {
                    // Fallback to sysroot/lib if rustc_driver not found
                    let lib_path = format!("{}/lib:/usr/local/lib:/usr/lib", sysroot.display());
                    unsafe {
                        std::env::set_var("DYLD_FALLBACK_LIBRARY_PATH", &lib_path);
                    }
                    eprintln!(
                        "macOS ARM detected: Set DYLD_FALLBACK_LIBRARY_PATH={lib_path} (fallback)"
                    );
                }
            }
        }
    }

    simple_logger::SimpleLogger::new()
        .env()
        .with_colors(true)
        .init()
        .unwrap();
    exit(core::run_compiler())
}
