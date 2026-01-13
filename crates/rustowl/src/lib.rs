//! # RustOwl Library
//!
//! RustOwl is a Language Server Protocol (LSP) implementation for visualizing
//! ownership and lifetimes in Rust code.
//!
//! The core analysis is performed by the `rustowlc` binary (a rustc wrapper).
//! This library provides the common data models and the LSP-side orchestration.
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

use std::io::IsTerminal;
use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};

use indicatif::ProgressBar;

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

use tracing_subscriber::{EnvFilter, filter::LevelFilter, fmt, prelude::*};

static ACTIVE_PROGRESS_BAR: OnceLock<Mutex<Option<ProgressBar>>> = OnceLock::new();

fn set_active_progress_bar(pb: Option<ProgressBar>) {
    let cell = ACTIVE_PROGRESS_BAR.get_or_init(|| Mutex::new(None));
    *cell.lock().expect("progress bar mutex poisoned") = pb;
}

fn with_active_progress_bar<R>(f: impl FnOnce(Option<&ProgressBar>) -> R) -> R {
    let cell = ACTIVE_PROGRESS_BAR.get_or_init(|| Mutex::new(None));
    let guard = cell.lock().expect("progress bar mutex poisoned");
    f(guard.as_ref())
}

#[derive(Default, Clone, Copy)]
struct IndicatifOrStderrWriter;

impl<'a> fmt::MakeWriter<'a> for IndicatifOrStderrWriter {
    type Writer = IndicatifOrStderr;

    fn make_writer(&'a self) -> Self::Writer {
        IndicatifOrStderr
    }
}

struct IndicatifOrStderr;

impl Write for IndicatifOrStderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let msg = match std::str::from_utf8(buf) {
            Ok(v) => v,
            // If it's not valid UTF-8, fall back to raw stderr.
            Err(_) => return io::stderr().write(buf),
        };

        with_active_progress_bar(|pb| {
            if let Some(pb) = pb {
                for line in msg.lines() {
                    pb.println(line);
                }
                Ok(buf.len())
            } else {
                io::stderr().write_all(buf).map(|()| buf.len())
            }
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        with_active_progress_bar(|pb| {
            if pb.is_some() {
                Ok(())
            } else {
                io::stderr().flush()
            }
        })
    }
}

#[must_use]
pub struct ActiveProgressBarGuard {
    previous: Option<ProgressBar>,
}

impl ActiveProgressBarGuard {
    pub fn set(pb: ProgressBar) -> Self {
        let previous = ACTIVE_PROGRESS_BAR
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("progress bar mutex poisoned")
            .take();
        set_active_progress_bar(Some(pb));
        Self { previous }
    }
}

impl Drop for ActiveProgressBarGuard {
    fn drop(&mut self) {
        set_active_progress_bar(self.previous.take());
    }
}

/// Initializes the logging system with colors and a default log level.
///
/// If a global subscriber is already set (e.g. by another binary), this
/// silently returns without re-initializing.
pub fn initialize_logging(level: LevelFilter) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default: show only rustowl logs at the requested level to avoid
        // drowning users in dependency logs.
        EnvFilter::new(format!("rustowl={level}"))
    });

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_writer(IndicatifOrStderrWriter)
        .with_ansi(std::io::stderr().is_terminal());

    // Ignore error if already initialized
    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init();
}

/// Test utilities for Miri-compatible async tests.
///
/// Miri doesn't support `#[tokio::test]` directly, so we provide a macro
/// that handles the async runtime setup correctly for both regular tests
/// and Miri.
///
/// See: <https://github.com/rust-lang/miri/issues/602#issuecomment-884019764>
#[macro_export]
macro_rules! async_test {
    ($name:ident, $body:expr) => {
        #[test]
        #[cfg_attr(miri, ignore)]
        fn $name() {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on($body)
        }
    };
}

// Miri tests finding UB (Undefined Behaviour)
mod miri_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use indicatif::ProgressBar;

    #[test]
    fn active_progress_bar_guard_restores_previous_progress_bar() {
        let pb1 = ProgressBar::hidden();
        let pb2 = ProgressBar::hidden();

        let _guard1 = ActiveProgressBarGuard::set(pb1.clone());
        super::with_active_progress_bar(|pb| {
            assert!(pb.is_some());
        });

        {
            let _guard2 = ActiveProgressBarGuard::set(pb2.clone());
            super::with_active_progress_bar(|pb| {
                assert!(pb.is_some());
            });
        }

        super::with_active_progress_bar(|pb| {
            assert!(pb.is_some());
        });

        drop(_guard1);

        super::with_active_progress_bar(|pb| {
            assert!(pb.is_none());
        });
    }

    #[test]
    fn initialize_logging_is_idempotent() {
        initialize_logging(tracing_subscriber::filter::LevelFilter::DEBUG);
        initialize_logging(tracing_subscriber::filter::LevelFilter::INFO);
    }
}
