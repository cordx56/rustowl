//! # RustOwl lib
//!
//! Libraries that used in RustOwl

pub mod cache;
pub mod cli;
pub mod lsp;
pub mod models;
pub mod shells;
pub mod toolchain;
pub mod utils;

pub use lsp::backend::Backend;

// Miri-specific memory safety tests
#[cfg(test)]
mod miri_tests;
