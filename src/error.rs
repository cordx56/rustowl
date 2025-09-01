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
}
