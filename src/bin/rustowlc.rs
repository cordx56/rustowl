//! # RustOwl rustowlc
//!
//! A compiler implementation for visualizing ownership and lifetimes in Rust, designed for debugging and optimization.

#![feature(rustc_private)]

pub extern crate polonius_engine;
pub extern crate rustc_borrowck;
pub extern crate rustc_data_structures;
pub extern crate rustc_driver;
pub extern crate rustc_hir;
pub extern crate rustc_index;
pub extern crate rustc_interface;
pub extern crate rustc_middle;
pub extern crate rustc_mir_dataflow;
pub extern crate rustc_session;
pub extern crate rustc_span;
pub extern crate rustc_stable_hash;
pub extern crate rustc_type_ir;

#[rustversion::before(1.95.0)]
pub extern crate rustc_query_system;

pub mod core;

// Cited from rustc
// https://github.com/rust-lang/rust/pull/148925
// MIT License
#[cfg(any(target_os = "linux", target_os = "macos"))]
use tikv_jemalloc_sys as _;

fn main() -> std::process::ExitCode {
    simple_logger::SimpleLogger::new()
        .env()
        .with_colors(true)
        .init()
        .unwrap();

    // rayon panics without this only on Windows
    #[cfg(target_os = "windows")]
    {
        rayon::ThreadPoolBuilder::new()
            .stack_size(4 * 1024 * 1024)
            .build_global()
            .unwrap();
    }

    core::run_compiler()
}
