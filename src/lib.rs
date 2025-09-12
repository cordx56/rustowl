//! # RustOwl Library
//!
//! RustOwl is a Language Server Protocol (LSP) implementation for visualizing 
//! ownership and lifetimes in Rust code. This library provides the core 
//! functionality for analyzing Rust programs and extracting ownership information.
//!
//! ## Core Components
//!
//! - **LSP Backend**: Language server implementation for IDE integration
//! - **Analysis Engine**: Rust compiler integration for ownership analysis  
//! - **Caching System**: Intelligent caching for improved performance
//! - **Error Handling**: Comprehensive error reporting with context
//! - **Toolchain Management**: Automatic setup and management of analysis tools
//!
//! ## Usage
//!
//! This library is primarily used by the RustOwl binary for LSP server functionality,
//! but can also be used directly for programmatic analysis of Rust code.

/// Core caching functionality for analysis results
pub mod cache;
/// Command-line interface definitions
pub mod cli;
/// Comprehensive error handling with context
pub mod error;
/// Language Server Protocol implementation
pub mod lsp;
/// Data models for analysis results
pub mod models;
/// Shell completion utilities
pub mod shells;
/// Rust toolchain management
pub mod toolchain;
/// General utility functions
pub mod utils;

pub use lsp::backend::Backend;

// Miri-specific memory safety tests
#[cfg(test)]
mod miri_tests;
