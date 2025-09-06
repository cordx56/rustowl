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

use std::io::IsTerminal;

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

/// Initializes the logging system with colors and a default log level.
///
/// If a global subscriber is already set (e.g. by another binary), this
/// silently returns without re-initializing.
pub fn initialize_logging(level: LevelFilter) {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.to_string()));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal());

    // Ignore error if already initialized
    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .try_init();
}

// Miri-specific memory safety tests
#[cfg(test)]
mod miri_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_structure() {
        // Test that all modules are accessible and key types can be imported
        use crate::cache::CacheConfig;
        use crate::error::RustOwlError;
        use crate::models::{FnLocal, Loc, Range};
        use crate::shells::Shell;

        // Test basic construction of key types
        let _config = CacheConfig::default();
        let _fn_local = FnLocal::new(1, 2);
        let _loc = Loc(10);
        let _range = Range::new(Loc(0), Loc(5));
        let _shell = Shell::Bash;

        // Test error types
        let _error = RustOwlError::Cache("test error".to_string());

        // Verify Backend type is available
        let _backend_type = std::any::type_name::<Backend>();
    }

    #[test]
    fn test_public_api() {
        // Test that the public API exports work correctly

        // Backend should be available from root
        let backend_name = std::any::type_name::<Backend>();
        assert!(backend_name.contains("Backend"));

        // Test that modules contain expected items
        use crate::models::*;
        use crate::utils::*;

        // Test utils functions
        let range1 = Range::new(Loc(0), Loc(10)).unwrap();
        let range2 = Range::new(Loc(5), Loc(15)).unwrap();

        assert!(common_range(range1, range2).is_some());

        // Test models
        let mut variables = MirVariables::new();
        let var = MirVariable::User {
            index: 1,
            live: range1,
            dead: range2,
        };
        variables.push(var);

        let vec = variables.to_vec();
        assert_eq!(vec.len(), 1);
    }

    #[test]
    fn test_type_compatibility() {
        // Test that types work together as expected in the public API
        use crate::models::*;
        use crate::utils::*;

        // Create a function with basic blocks
        let mut function = Function::new(42);

        // Add a basic block
        let mut bb = MirBasicBlock::new();
        bb.statements.push(MirStatement::Other {
            range: Range::new(Loc(0), Loc(5)).unwrap(),
        });
        function.basic_blocks.push(bb);

        // Test visitor pattern
        struct CountingVisitor {
            count: usize,
        }

        impl MirVisitor for CountingVisitor {
            /// Increment the visitor's internal count when a function node is visited.
            ///
            /// This method is invoked for each function encountered during MIR traversal.
            /// It does not inspect the function; it only records that a function visit occurred.
            ///
            /// # Examples
            ///
            /// ```no_run
            /// let mut visitor = CountingVisitor { count: 0 };
            /// let func = /* obtain a `Function` reference from the MIR being visited */ unimplemented!();
            /// visitor.visit_func(&func);
            /// assert_eq!(visitor.count, 1);
            /// ```
            fn visit_func(&mut self, _func: &Function) {
                self.count += 1;
            }

            /// Increment the visitor's statement counter by one.
            ///
            /// This is called for each `MirStatement` visited; it tracks how many statements
            /// the visitor has seen by incrementing `self.count`.
            ///
            /// # Examples
            ///
            /// ```
            /// use crate::models::{MirStatement, Range, Loc};
            ///
            /// let mut visitor = CountingVisitor { count: 0 };
            /// let stmt = MirStatement::Other { range: Range::new(Loc(0), Loc(1)).unwrap() };
            /// visitor.visit_stmt(&stmt);
            /// assert_eq!(visitor.count, 1);
            /// ```
            fn visit_stmt(&mut self, _stmt: &MirStatement) {
                self.count += 1;
            }
        }

        let mut visitor = CountingVisitor { count: 0 };
        mir_visit(&function, &mut visitor);

        assert_eq!(visitor.count, 2); // 1 function + 1 statement
    }

    #[test]
    fn test_initialize_logging_multiple_calls() {
        // Test that multiple calls to initialize_logging are safe
        use tracing_subscriber::filter::LevelFilter;

        initialize_logging(LevelFilter::INFO);
        initialize_logging(LevelFilter::DEBUG); // Should not panic
        initialize_logging(LevelFilter::WARN); // Should not panic
    }

    #[test]
    fn test_initialize_logging_different_levels() {
        // Test initialization with different log levels
        use tracing_subscriber::filter::LevelFilter;

        // Test all supported levels
        let levels = [
            LevelFilter::OFF,
            LevelFilter::ERROR,
            LevelFilter::WARN,
            LevelFilter::INFO,
            LevelFilter::DEBUG,
            LevelFilter::TRACE,
        ];

        for level in levels {
            // Each call should complete without panicking
            initialize_logging(level);
        }
    }

    #[test]
    fn test_module_re_exports() {
        // Test that re-exports work correctly
        use crate::Backend;

        // Backend should be accessible from the root module
        let type_name = std::any::type_name::<Backend>();
        assert!(type_name.contains("Backend"));
        assert!(type_name.contains("rustowl"));
    }

    #[test]
    fn test_public_module_access() {
        // Test that all public modules are accessible
        use crate::{cache, error, models, shells, utils};

        // Test basic functionality from each module
        let _cache_config = cache::CacheConfig::default();
        let _shell = shells::Shell::Bash;
        let _error = error::RustOwlError::Cache("test".to_string());
        let _loc = models::Loc(42);

        // Test utils functions
        let range1 = models::Range::new(models::Loc(0), models::Loc(5)).unwrap();
        let range2 = models::Range::new(models::Loc(3), models::Loc(8)).unwrap();
        assert!(utils::common_range(range1, range2).is_some());
    }

    #[test]
    fn test_error_types_integration() {
        // Test error handling integration across modules
        use crate::error::RustOwlError;

        let errors = [
            RustOwlError::Cache("cache error".to_string()),
            RustOwlError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test")),
            RustOwlError::Json(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err()),
            RustOwlError::Toolchain("toolchain error".to_string()),
        ];

        for error in errors {
            // Each error should display properly
            let display = format!("{error}");
            assert!(!display.is_empty());

            // Each error should have a source (for some types)
            let _source = std::error::Error::source(&error);
        }
    }

    #[test]
    fn test_data_model_serialization() {
        // Test that data models can be serialized/deserialized
        use crate::models::*;

        // Test basic types
        let loc = Loc(42);
        let range = Range::new(Loc(0), Loc(10)).unwrap();
        let fn_local = FnLocal::new(1, 2);

        // Test serialization (implicitly tests serde derives)
        let loc_json = serde_json::to_string(&loc).unwrap();
        let range_json = serde_json::to_string(&range).unwrap();
        let fn_local_json = serde_json::to_string(&fn_local).unwrap();

        // Test deserialization
        let _loc_back: Loc = serde_json::from_str(&loc_json).unwrap();
        let _range_back: Range = serde_json::from_str(&range_json).unwrap();
        let _fn_local_back: FnLocal = serde_json::from_str(&fn_local_json).unwrap();
    }

    #[test]
    fn test_complex_data_structures() {
        // Test creation and manipulation of complex nested structures
        use crate::models::*;

        // Create a workspace with multiple crates
        let mut workspace = Workspace(FoldIndexMap::default());

        let mut crate1 = Crate(FoldIndexMap::default());
        let mut file1 = File::new();

        let mut function = Function::new(1);
        let mut basic_block = MirBasicBlock::new();

        // Add statements to basic block
        basic_block.statements.push(MirStatement::Other {
            range: Range::new(Loc(0), Loc(5)).unwrap(),
        });

        function.basic_blocks.push(basic_block);
        file1.items.push(function);
        crate1.0.insert("src/lib.rs".to_string(), file1);
        workspace.0.insert("lib1".to_string(), crate1);

        // Verify structure integrity
        assert_eq!(workspace.0.len(), 1);
        assert!(workspace.0.contains_key("lib1"));

        let crate_ref = workspace.0.get("lib1").unwrap();
        assert_eq!(crate_ref.0.len(), 1);
        assert!(crate_ref.0.contains_key("src/lib.rs"));

        let file_ref = crate_ref.0.get("src/lib.rs").unwrap();
        assert_eq!(file_ref.items.len(), 1);

        let func_ref = &file_ref.items[0];
        assert_eq!(func_ref.basic_blocks.len(), 1);
        assert_eq!(func_ref.basic_blocks[0].statements.len(), 1);
    }
}
