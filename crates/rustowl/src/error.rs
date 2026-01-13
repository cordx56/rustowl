//! Error handling for RustOwl using anyhow for flexible error handling.

pub use anyhow::{Context, Result, anyhow, bail};

/// Main error type for RustOwl operations.
/// Used for typed errors that need to be matched on.
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

impl std::fmt::Display for RustOwlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
        assert!(matches!(rustowl_error, RustOwlError::Io(_)));

        let json_str = "{ invalid json";
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let rustowl_error: RustOwlError = json_error.into();
        assert!(matches!(rustowl_error, RustOwlError::Json(_)));
    }
}
