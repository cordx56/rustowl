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
pub mod visualize;

pub use lsp::backend::Backend;

use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;

/// Level used when a process queries the effective level before initializing logging.
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::INFO;

/// The level actually installed by [`initialize_logging`], for subprocess propagation.
static EFFECTIVE_LEVEL: OnceLock<LevelFilter> = OnceLock::new();

/// The maximum level this process will emit, as configured by [`initialize_logging`].
pub fn effective_log_level() -> LevelFilter {
    *EFFECTIVE_LEVEL.get().unwrap_or(&DEFAULT_LOG_LEVEL)
}

/// Renders a level as an `RUST_LOG` directive scoped to this crate's binaries.
pub fn log_filter_directive(level: LevelFilter) -> String {
    format!("rustowl={level},rustowlc={level}")
}

/// Resolves the log level for a run from the parsed verbosity flags.
pub fn log_level_for<L: clap_verbosity_flag::LogLevel>(
    verbosity: &clap_verbosity_flag::Verbosity<L>,
    lsp_mode: bool,
) -> LevelFilter {
    if lsp_mode && !verbosity.is_present() {
        LevelFilter::WARN
    } else {
        verbosity.tracing_level_filter()
    }
}

/// Installs the global `tracing` subscriber.
pub fn initialize_logging(level: LevelFilter) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_filter_directive(level)));

    let _ = EFFECTIVE_LEVEL.set(level);

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_writer(std::io::stderr);

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init();
}

// Miri-specific memory safety tests
#[cfg(test)]
mod miri_tests;
