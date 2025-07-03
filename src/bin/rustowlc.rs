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
        {
            unsafe extern "C" {
                fn _mi_macros_override_malloc();
            }

            #[used]
            static _F7: unsafe extern "C" fn() = _mi_macros_override_malloc;
        }
    }

    simple_logger::SimpleLogger::new()
        .env()
        .with_colors(true)
        .init()
        .unwrap();
    exit(core::run_compiler())
}
