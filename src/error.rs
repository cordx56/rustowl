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