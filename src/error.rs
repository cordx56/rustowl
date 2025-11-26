//! Error handling for RustOwl using the eros crate for context-aware error handling.

use std::fmt;

/// Main error type for RustOwl operations
#[derive(Debug)]
pub enum RustOwlError {
    /// I/O operation failed
    Io(std::io::Error),
    /// Cargo metadata operation failed
    CargoMetadata(String),
    /// Toolchain operation failed
    Toolchain(String),
    /// JSON serialization/deserialization failed
    Json(serde_json::Error),
    /// Cache operation failed
    Cache(String),
    /// LSP operation failed
    Lsp(String),
    /// General analysis error
    Analysis(String),
    /// Configuration error
    Config(String),
}

impl fmt::Display for RustOwlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RustOwlError::Io(err) => write!(f, "I/O error: {err}"),
            RustOwlError::CargoMetadata(msg) => write!(f, "Cargo metadata error: {msg}"),
            RustOwlError::Toolchain(msg) => write!(f, "Toolchain error: {msg}"),
            RustOwlError::Json(err) => write!(f, "JSON error: {err}"),
            RustOwlError::Cache(msg) => write!(f, "Cache error: {msg}"),
            RustOwlError::Lsp(msg) => write!(f, "LSP error: {msg}"),
            RustOwlError::Analysis(msg) => write!(f, "Analysis error: {msg}"),
            RustOwlError::Config(msg) => write!(f, "Configuration error: {msg}"),
        }
    }
}

impl std::error::Error for RustOwlError {}

impl From<std::io::Error> for RustOwlError {
    fn from(err: std::io::Error) -> Self {
        RustOwlError::Io(err)
    }
}

impl From<serde_json::Error> for RustOwlError {
    fn from(err: serde_json::Error) -> Self {
        RustOwlError::Json(err)
    }
}

/// Result type for RustOwl operations
pub type Result<T> = std::result::Result<T, RustOwlError>;

/// Extension trait for adding context to results
pub trait ErrorContext<T> {
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    fn context(self, msg: &str) -> Result<T>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|_| RustOwlError::Analysis(f()))
    }

    fn context(self, msg: &str) -> Result<T> {
        self.with_context(|| msg.to_string())
    }
}

impl<T> ErrorContext<T> for Option<T> {
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| RustOwlError::Analysis(f()))
    }

    fn context(self, msg: &str) -> Result<T> {
        self.with_context(|| msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rustowl_error_display() {
        let io_err = RustOwlError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(io_err.to_string().contains("I/O error"));

        let cargo_err = RustOwlError::CargoMetadata("invalid metadata".to_string());
        assert_eq!(
            cargo_err.to_string(),
            "Cargo metadata error: invalid metadata"
        );

        let toolchain_err = RustOwlError::Toolchain("setup failed".to_string());
        assert_eq!(toolchain_err.to_string(), "Toolchain error: setup failed");
    }

    #[test]
    fn test_error_from_conversions() {
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let rustowl_error: RustOwlError = io_error.into();
        match rustowl_error {
            RustOwlError::Io(_) => {}
            _ => panic!("Expected Io variant"),
        }

        // Test with a real JSON error by trying to parse invalid JSON
        let json_str = "{ invalid json";
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let rustowl_error: RustOwlError = json_error.into();
        match rustowl_error {
            RustOwlError::Json(_) => {}
            _ => panic!("Expected Json variant"),
        }
    }

    #[test]
    fn test_error_context_trait() {
        // Test with io::Error which implements std::error::Error
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let result: std::result::Result<i32, std::io::Error> = Err(io_error);
        let with_context = result.context("additional context");

        assert!(with_context.is_err());
        match with_context {
            Err(RustOwlError::Analysis(msg)) => assert_eq!(msg, "additional context"),
            _ => panic!("Expected Analysis error with context"),
        }

        let option: Option<i32> = None;
        let with_context = option.context("option was None");

        assert!(with_context.is_err());
        match with_context {
            Err(RustOwlError::Analysis(msg)) => assert_eq!(msg, "option was None"),
            _ => panic!("Expected Analysis error with context"),
        }
    }

    #[test]
    fn test_error_context_with_closure() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let result: std::result::Result<i32, std::io::Error> = Err(io_error);
        let with_context = result.with_context(|| "dynamic context".to_string());

        assert!(with_context.is_err());
        match with_context {
            Err(RustOwlError::Analysis(msg)) => assert_eq!(msg, "dynamic context"),
            _ => panic!("Expected Analysis error with dynamic context"),
        }
    }

    #[test]
    fn test_all_error_variants_display() {
        // Test display for all error variants
        let errors = vec![
            RustOwlError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test")),
            RustOwlError::CargoMetadata("metadata failed".to_string()),
            RustOwlError::Toolchain("toolchain setup failed".to_string()),
            RustOwlError::Json(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err()),
            RustOwlError::Cache("cache write failed".to_string()),
            RustOwlError::Lsp("lsp connection failed".to_string()),
            RustOwlError::Analysis("analysis failed".to_string()),
            RustOwlError::Config("config parse failed".to_string()),
        ];

        for error in errors {
            let display_str = error.to_string();
            assert!(!display_str.is_empty());

            // Each error type should have a descriptive prefix
            match error {
                RustOwlError::Io(_) => assert!(display_str.starts_with("I/O error:")),
                RustOwlError::CargoMetadata(_) => {
                    assert!(display_str.starts_with("Cargo metadata error:"))
                }
                RustOwlError::Toolchain(_) => assert!(display_str.starts_with("Toolchain error:")),
                RustOwlError::Json(_) => assert!(display_str.starts_with("JSON error:")),
                RustOwlError::Cache(_) => assert!(display_str.starts_with("Cache error:")),
                RustOwlError::Lsp(_) => assert!(display_str.starts_with("LSP error:")),
                RustOwlError::Analysis(_) => assert!(display_str.starts_with("Analysis error:")),
                RustOwlError::Config(_) => assert!(display_str.starts_with("Configuration error:")),
            }
        }
    }

    #[test]
    fn test_error_debug_implementation() {
        let error = RustOwlError::Toolchain("test error".to_string());
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("Toolchain"));
        assert!(debug_str.contains("test error"));
    }

    #[test]
    fn test_std_error_trait() {
        let error = RustOwlError::Analysis("test analysis error".to_string());

        // Test that it implements std::error::Error
        let std_error: &dyn std::error::Error = &error;
        assert_eq!(std_error.to_string(), "Analysis error: test analysis error");

        // Test source() method (should return None for our simple errors)
        assert!(std_error.source().is_none());
    }

    #[test]
    fn test_error_from_conversions_comprehensive() {
        // Test various I/O error kinds
        let io_errors = vec![
            std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
            std::io::Error::new(std::io::ErrorKind::AlreadyExists, "already exists"),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid input"),
            std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout"),
        ];

        for io_error in io_errors {
            let rustowl_error: RustOwlError = io_error.into();
            match rustowl_error {
                RustOwlError::Io(_) => {}
                _ => panic!("Expected Io variant"),
            }
        }

        // Test various JSON errors
        let json_test_cases = vec![
            "{ invalid json",
            "[1, 2, invalid",
            "\"unterminated string",
            "{ \"key\": }", // missing value
        ];

        for test_case in json_test_cases {
            let json_error = serde_json::from_str::<serde_json::Value>(test_case).unwrap_err();
            let rustowl_error: RustOwlError = json_error.into();
            match rustowl_error {
                RustOwlError::Json(_) => {}
                _ => panic!("Expected Json variant for test case: {test_case}"),
            }
        }
    }

    #[test]
    fn test_result_type_alias() {
        // Test that our Result type alias works correctly
        fn test_function() -> Result<i32> {
            Ok(42)
        }

        fn test_function_error() -> Result<i32> {
            Err(RustOwlError::Analysis("test error".to_string()))
        }

        assert_eq!(test_function().unwrap(), 42);
        assert!(test_function_error().is_err());

        // Test chaining
        let result = test_function().map(|x| x * 2).map(|x| x + 1);
        assert_eq!(result.unwrap(), 85);
    }

    #[test]
    fn test_error_context_chaining() {
        // Test chaining multiple context operations
        let option: Option<i32> = None;
        let result = option.context("first context");

        assert!(result.is_err());
        match result {
            Err(RustOwlError::Analysis(msg)) => assert_eq!(msg, "first context"),
            _ => panic!("Expected Analysis error"),
        }

        // Test successful operation with context chaining
        let option: Option<i32> = Some(42);
        let result = option.context("should not be used").map(|x| x * 2);
        assert_eq!(result.unwrap(), 84);
    }

    #[test]
    fn test_error_context_with_successful_operations() {
        // Test that context doesn't interfere with successful operations
        let result: std::result::Result<i32, std::io::Error> = Ok(42);
        let with_context = result.context("this context should not be used");
        assert_eq!(with_context.unwrap(), 42);

        let option: Option<i32> = Some(100);
        let with_context = option.context("this context should not be used");
        assert_eq!(with_context.unwrap(), 100);
    }

    #[test]
    fn test_error_context_with_complex_types() {
        // Test context with more complex error types
        use std::num::ParseIntError;

        let parse_result: std::result::Result<i32, ParseIntError> = "not_a_number".parse();
        let with_context = parse_result.context("failed to parse number");

        assert!(with_context.is_err());
        match with_context {
            Err(RustOwlError::Analysis(msg)) => assert_eq!(msg, "failed to parse number"),
            _ => panic!("Expected Analysis error"),
        }
    }

    #[test]
    fn test_error_context_dynamic_messages() {
        // Test with_context with dynamic message generation
        let counter = 5;
        let result: std::result::Result<i32, std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));

        let with_context = result.with_context(|| format!("operation {counter} failed"));

        assert!(with_context.is_err());
        match with_context {
            Err(RustOwlError::Analysis(msg)) => assert_eq!(msg, "operation 5 failed"),
            _ => panic!("Expected Analysis error"),
        }
    }

    #[test]
    fn test_error_variant_construction() {
        // Test direct construction of error variants
        let errors = vec![
            RustOwlError::CargoMetadata("custom metadata error".to_string()),
            RustOwlError::Toolchain("custom toolchain error".to_string()),
            RustOwlError::Cache("custom cache error".to_string()),
            RustOwlError::Lsp("custom lsp error".to_string()),
            RustOwlError::Analysis("custom analysis error".to_string()),
            RustOwlError::Config("custom config error".to_string()),
        ];

        for error in errors {
            // Verify each error can be created and has the expected message
            let message = error.to_string();
            assert!(!message.is_empty());
            assert!(message.contains("custom"));
            assert!(message.contains("error"));
        }
    }

    #[test]
    fn test_error_send_sync() {
        // Test that our error type implements Send and Sync
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<RustOwlError>();
        assert_sync::<RustOwlError>();

        // Test that we can pass errors across threads (conceptually)
        let error = RustOwlError::Analysis("thread test".to_string());
        let error_clone = format!("{error}"); // This would work across threads
        assert!(!error_clone.is_empty());
    }

    #[test]
    fn test_error_context_trait_generic_bounds() {
        // Test that ErrorContext works with various error types that implement std::error::Error

        // Test with a custom error type
        #[derive(Debug)]
        struct CustomError;

        impl std::fmt::Display for CustomError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "custom error")
            }
        }

        impl std::error::Error for CustomError {}

        let custom_result: std::result::Result<i32, CustomError> = Err(CustomError);
        let with_context = custom_result.context("custom error context");

        assert!(with_context.is_err());
        match with_context {
            Err(RustOwlError::Analysis(msg)) => assert_eq!(msg, "custom error context"),
            _ => panic!("Expected Analysis error"),
        }
    }

    #[test]
    fn test_error_chain_comprehensive() {
        // Test error chaining with various error types
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let rustowl_error: RustOwlError = io_error.into();

        // Check that the original error information is preserved
        match rustowl_error {
            RustOwlError::Io(ref inner) => {
                assert_eq!(inner.kind(), std::io::ErrorKind::NotFound);
                assert!(inner.to_string().contains("file not found"));
            }
            _ => panic!("Expected Io variant"),
        }

        // Test JSON error chaining
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let rustowl_json_error: RustOwlError = json_error.into();

        match rustowl_json_error {
            RustOwlError::Json(ref inner) => {
                assert!(inner.to_string().contains("expected"));
            }
            _ => panic!("Expected Json variant"),
        }
    }

    #[test]
    fn test_send_sync_traits() {
        // Test that RustOwlError implements Send + Sync
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<RustOwlError>();
        assert_sync::<RustOwlError>();

        // Test that we can move errors across thread boundaries (conceptually)
        let error = RustOwlError::Cache("test".to_string());
        let boxed_error: Box<dyn std::error::Error + Send + Sync> = Box::new(error);

        // Should be able to downcast back
        if boxed_error.downcast::<RustOwlError>().is_ok() {
            // Successfully downcasted
        } else {
            panic!("Failed to downcast error");
        }
    }

    #[test]
    fn test_error_variant_exhaustiveness() {
        // Test all error variants to ensure they're handled
        let errors = vec![
            RustOwlError::Cache("cache".to_string()),
            RustOwlError::Io(std::io::Error::other("io")),
            RustOwlError::Json(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err()),
            RustOwlError::Toolchain("toolchain".to_string()),
            RustOwlError::Lsp("lsp".to_string()),
            RustOwlError::Analysis("analysis".to_string()),
            RustOwlError::Config("config".to_string()),
        ];

        for error in errors {
            // Each error should display properly
            let display = format!("{error}");
            assert!(!display.is_empty());

            // Each error should debug properly
            let debug = format!("{error:?}");
            assert!(!debug.is_empty());

            // Each error should implement std::error::Error
            let std_error: &dyn std::error::Error = &error;
            let error_string = std_error.to_string();
            assert!(!error_string.is_empty());
        }
    }

    #[test]
    fn test_error_context_with_complex_messages() {
        // Test context with complex error messages
        let long_message = "very ".repeat(100) + "long message";
        let complex_messages = vec![
            "simple message",
            "message with unicode: 🦀 rust",
            "message\nwith\nnewlines",
            "message with \"quotes\" and 'apostrophes'",
            "message with numbers: 123, 456.789",
            "message with special chars: !@#$%^&*()",
            "",            // Empty message
            &long_message, // Very long message
        ];

        for message in complex_messages {
            let result: std::result::Result<(), std::io::Error> =
                Err(std::io::Error::other("test error"));

            let with_context = result.context(message);
            assert!(with_context.is_err());

            match with_context {
                Err(RustOwlError::Analysis(ctx_msg)) => {
                    assert_eq!(ctx_msg, message);
                }
                _ => panic!("Expected Analysis error with context"),
            }
        }
    }

    #[test]
    fn test_error_memory_usage() {
        // Test that errors don't use excessive memory
        let error = RustOwlError::Cache("test".to_string());
        let size = std::mem::size_of_val(&error);

        // Error should be reasonably sized (less than a few KB)
        assert!(size < 1024, "Error size {size} bytes is too large");

        // Test with larger nested errors
        let large_io_error = std::io::Error::other(
            "error message that is quite long and contains lots of text to test memory usage patterns",
        );
        let large_rustowl_error: RustOwlError = large_io_error.into();
        let large_size = std::mem::size_of_val(&large_rustowl_error);

        // Should still be reasonable even with larger nested errors
        assert!(
            large_size < 2048,
            "Large error size {large_size} bytes is too large"
        );
    }

    #[test]
    fn test_result_type_alias_comprehensive() {
        // Test the Result<T> type alias
        fn returns_result() -> Result<i32> {
            Ok(42)
        }

        fn returns_error() -> Result<i32> {
            Err(RustOwlError::Cache("test error".to_string()))
        }

        // Test successful result
        match returns_result() {
            Ok(value) => assert_eq!(value, 42),
            Err(_) => panic!("Expected success"),
        }

        // Test error result
        match returns_error() {
            Ok(_) => panic!("Expected error"),
            Err(error) => match error {
                RustOwlError::Cache(msg) => assert_eq!(msg, "test error"),
                _ => panic!("Expected Cache error"),
            },
        }
    }

    #[test]
    fn test_error_serialization_compatibility() {
        // Test that errors work well with serialization frameworks (where applicable)
        let errors = vec![
            RustOwlError::Cache("serialization test".to_string()),
            RustOwlError::Analysis("another test".to_string()),
            RustOwlError::Toolchain("toolchain test".to_string()),
            RustOwlError::Config("config test".to_string()),
            RustOwlError::Lsp("lsp test".to_string()),
        ];

        for error in errors {
            // Test that errors can be converted to strings reliably
            let error_string = error.to_string();
            assert!(!error_string.is_empty());

            // Test that debug representation is stable
            let debug_string = format!("{error:?}");
            assert!(!debug_string.is_empty());
            // Debug representation may be different length than display

            // Test multiple conversions are consistent
            let error_string2 = error.to_string();
            assert_eq!(error_string, error_string2);
        }
    }

    #[test]
    fn test_error_context_with_complex_error_types() {
        // Test context with various complex error types
        use std::num::{ParseFloatError, ParseIntError};

        // Test with ParseIntError
        let parse_int_result: std::result::Result<i32, ParseIntError> = "not_a_number".parse();
        let with_context = parse_int_result.context("failed to parse integer");
        assert!(with_context.is_err());

        // Test with ParseFloatError
        let parse_float_result: std::result::Result<f64, ParseFloatError> = "not_a_float".parse();
        let with_context = parse_float_result.context("failed to parse float");
        assert!(with_context.is_err());

        // Test with UTF8 error simulation
        let invalid_utf8 = vec![0xC0];
        let utf8_result = std::str::from_utf8(&invalid_utf8);
        let with_context = utf8_result.context("invalid utf8 sequence");
        assert!(with_context.is_err());
    }

    #[test]
    fn test_error_downcast_patterns() {
        // Test error downcasting patterns
        let errors: Vec<Box<dyn std::error::Error + Send + Sync>> = vec![
            Box::new(RustOwlError::Cache("cache error".to_string())),
            Box::new(RustOwlError::Io(std::io::Error::other("io error"))),
            Box::new(RustOwlError::Analysis("analysis error".to_string())),
        ];

        for boxed_error in errors {
            // Test downcasting to RustOwlError
            if let Ok(rustowl_error) = boxed_error.downcast::<RustOwlError>() {
                match *rustowl_error {
                    RustOwlError::Cache(_) | RustOwlError::Io(_) | RustOwlError::Analysis(_) => {
                        // Successfully downcasted
                    }
                    _ => panic!("Unexpected error variant"),
                }
            } else {
                panic!("Failed to downcast to RustOwlError");
            }
        }
    }

    #[test]
    fn test_error_source_chain_traversal() {
        // Test error source chain traversal
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "underlying cause");
        let rustowl_error: RustOwlError = io_error.into();

        // Test source method
        if let RustOwlError::Io(ref inner_io) = rustowl_error {
            assert_eq!(inner_io.kind(), std::io::ErrorKind::NotFound);
            assert!(inner_io.to_string().contains("underlying cause"));
        }

        // Test error chain traversal
        let std_error: &dyn std::error::Error = &rustowl_error;
        let mut error_chain = Vec::new();
        let mut current_error = Some(std_error);

        while let Some(error) = current_error {
            error_chain.push(error.to_string());
            current_error = error.source();
        }

        assert!(!error_chain.is_empty());
        assert!(error_chain[0].contains("I/O error"));
    }

    #[test]
    fn test_error_context_trait_bounds_comprehensive() {
        // Test that ErrorContext works with all expected trait bounds

        // Create a custom error that implements the required traits
        #[derive(Debug)]
        struct TestError {
            message: String,
        }

        impl std::fmt::Display for TestError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "TestError: {}", self.message)
            }
        }

        impl std::error::Error for TestError {}

        // Test Send + Sync bounds
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TestError>();

        let test_error = TestError {
            message: "test message".to_string(),
        };
        let result: std::result::Result<(), TestError> = Err(test_error);
        let with_context = result.context("additional context");

        assert!(with_context.is_err());
        match with_context {
            Err(RustOwlError::Analysis(msg)) => assert_eq!(msg, "additional context"),
            _ => panic!("Expected Analysis error"),
        }
    }

    #[test]
    fn test_error_variant_memory_efficiency() {
        // Test memory efficiency of different error variants
        use std::mem;

        let variants = vec![
            RustOwlError::Cache("test".to_string()),
            RustOwlError::Analysis("test".to_string()),
            RustOwlError::Toolchain("test".to_string()),
            RustOwlError::Config("test".to_string()),
            RustOwlError::Lsp("test".to_string()),
            RustOwlError::CargoMetadata("test".to_string()),
            RustOwlError::Io(std::io::Error::other("test")),
            RustOwlError::Json(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err()),
        ];

        for variant in variants {
            let size = mem::size_of_val(&variant);

            // Error should be reasonably sized
            assert!(
                size < 1024,
                "Error variant {:?} is too large: {} bytes",
                mem::discriminant(&variant),
                size
            );

            // Test that errors don't grow significantly with content
            let large_message = "x".repeat(1000);
            let large_variant = match variant {
                RustOwlError::Cache(_) => RustOwlError::Cache(large_message),
                RustOwlError::Analysis(_) => RustOwlError::Analysis(large_message),
                RustOwlError::Toolchain(_) => RustOwlError::Toolchain(large_message),
                RustOwlError::Config(_) => RustOwlError::Config(large_message),
                RustOwlError::Lsp(_) => RustOwlError::Lsp(large_message),
                RustOwlError::CargoMetadata(_) => RustOwlError::CargoMetadata(large_message),
                _ => continue, // Skip variants that don't contain strings
            };

            let large_size = mem::size_of_val(&large_variant);

            // Size of enum variants should be consistent regardless of string content
            // (since strings are heap-allocated)
            assert_eq!(
                large_size, size,
                "Enum size should be consistent for heap-allocated strings"
            );
            assert!(
                large_size < 2048,
                "Even large variants should be reasonable: {large_size} bytes"
            );
        }
    }

    #[test]
    fn test_error_formatting_edge_cases() {
        // Test error formatting with edge cases
        let edge_case_messages = vec![
            "",                                       // Empty string
            " ",                                      // Single space
            "\n",                                     // Single newline
            "\t",                                     // Single tab
            "🦀",                                     // Single emoji
            "test\0null",                             // Null character
            "very long message",                      // Very long message
            "unicode: 你好世界 🌍 здравствуй мир",    // Mixed unicode
            "quotes: \"double\" 'single' `backtick`", // Various quotes
            "special: !@#$%^&*()_+-=[]{}|;:,.<>?",    // Special characters
            "escaped: \\n \\t \\r \\\\",              // Escaped sequences
            "\u{200B}\u{FEFF}invisible",              // Zero-width characters
        ];

        for message in edge_case_messages {
            let errors = vec![
                RustOwlError::Cache(message.to_string()),
                RustOwlError::Analysis(message.to_string()),
                RustOwlError::Toolchain(message.to_string()),
                RustOwlError::Config(message.to_string()),
                RustOwlError::Lsp(message.to_string()),
                RustOwlError::CargoMetadata(message.to_string()),
            ];

            for error in errors {
                // Display should not panic
                let display_str = error.to_string();
                assert!(!display_str.is_empty() || message.is_empty());

                // Debug should not panic
                let debug_str = format!("{error:?}");
                assert!(!debug_str.is_empty());

                // Should contain the message (unless empty)
                if !message.is_empty() {
                    assert!(
                        display_str.contains(message),
                        "Display should contain message for: {message:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_error_thread_safety_comprehensive() {
        // Test comprehensive thread safety
        use std::sync::{Arc, Barrier};
        use std::thread;

        // Test that errors can be shared across threads
        let error = Arc::new(RustOwlError::Cache("shared error".to_string()));
        let barrier = Arc::new(Barrier::new(3));

        let handles: Vec<_> = (0..2)
            .map(|i| {
                let error_clone = Arc::clone(&error);
                let barrier_clone = Arc::clone(&barrier);

                thread::spawn(move || {
                    barrier_clone.wait();

                    // Each thread should be able to access the error
                    let error_str = error_clone.to_string();
                    assert!(error_str.contains("shared error"));

                    // Create new errors in thread
                    let thread_error = RustOwlError::Analysis(format!("thread {i} error"));
                    assert!(thread_error.to_string().contains(&format!("thread {i}")));

                    thread_error
                })
            })
            .collect();

        barrier.wait(); // Synchronize all threads

        // Collect results
        let thread_errors: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(thread_errors.len(), 2);
        for (i, error) in thread_errors.iter().enumerate() {
            assert!(error.to_string().contains(&format!("thread {i}")));
        }
    }

    #[test]
    fn test_error_conversion_completeness() {
        // Test comprehensive error conversions

        // Test all From implementations
        let io_error =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let rustowl_from_io: RustOwlError = io_error.into();
        match rustowl_from_io {
            RustOwlError::Io(_) => (),
            _ => panic!("Expected Io variant"),
        }

        let json_error = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
        let rustowl_from_json: RustOwlError = json_error.into();
        match rustowl_from_json {
            RustOwlError::Json(_) => (),
            _ => panic!("Expected Json variant"),
        }

        // Test manual construction of all variants
        let all_variants = vec![
            RustOwlError::Io(std::io::Error::other("io")),
            RustOwlError::CargoMetadata("cargo".to_string()),
            RustOwlError::Toolchain("toolchain".to_string()),
            RustOwlError::Json(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err()),
            RustOwlError::Cache("cache".to_string()),
            RustOwlError::Lsp("lsp".to_string()),
            RustOwlError::Analysis("analysis".to_string()),
            RustOwlError::Config("config".to_string()),
        ];

        for error in all_variants {
            // Each variant should implement required traits
            let _display: String = error.to_string();
            let _debug: String = format!("{error:?}");
            let _std_error: &dyn std::error::Error = &error;
        }
    }
}
