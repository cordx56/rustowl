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

fn main() {
    // This is cited from [rustc](https://github.com/rust-lang/rust/blob/1.86.0/compiler/rustc/src/main.rs).
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
            extern "C" {
                fn _rjem_je_zone_register();
                        }

            #[used]
            static _F7: unsafe extern "C" fn() = _rjem_je_zone_register;
        }
    }

    simple_logger::SimpleLogger::new()
        .env()
        .with_colors(true)
        .init()
        .unwrap();
    exit(core::run_compiler())
}
