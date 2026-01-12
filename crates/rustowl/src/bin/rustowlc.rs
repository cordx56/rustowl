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

// Cited from rustc https://github.com/rust-lang/rust/blob/73cecf3a39bfb5a57982311de238147dd1c34a1f/compiler/rustc/src/main.rs
// MIT License
#[cfg(any(target_os = "linux", target_os = "macos"))]
use tikv_jemalloc_sys as _;

pub mod core;

use std::process::exit;

fn main() {
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
