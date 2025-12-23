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
        match rustowl_error {
            RustOwlError::Io(_) => {}
            _ => panic!("Expected Io variant"),
        }

        let json_str = "{ invalid json";
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let rustowl_error: RustOwlError = json_error.into();
        match rustowl_error {
            RustOwlError::Json(_) => {}
            _ => panic!("Expected Json variant"),
        }
    }

    #[test]
    fn test_anyhow_context() {
        fn might_fail() -> Result<i32> {
            let result: std::result::Result<i32, std::io::Error> =
                Err(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
            result.context("failed to do something")
        }

        let err = might_fail().unwrap_err();
        assert!(err.to_string().contains("failed to do something"));
    }

    #[test]
    fn test_anyhow_bail() {
        fn always_fails() -> Result<()> {
            bail!("this always fails")
        }

        let err = always_fails().unwrap_err();
        assert!(err.to_string().contains("this always fails"));
    }

    #[test]
    fn test_anyhow_anyhow_macro() {
        fn create_error() -> Result<()> {
            Err(anyhow!("dynamic error: {}", 42))
        }

        let err = create_error().unwrap_err();
        assert!(err.to_string().contains("dynamic error: 42"));
    }

    #[test]
    fn test_all_error_variants_display() {
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
        let std_error: &dyn std::error::Error = &error;
        assert_eq!(std_error.to_string(), "Analysis error: test analysis error");
    }

    #[test]
    fn test_send_sync_traits() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<RustOwlError>();
        assert_sync::<RustOwlError>();
    }

    #[test]
    fn test_result_type_alias() {
        fn test_function() -> Result<i32> {
            Ok(42)
        }

        fn test_function_error() -> Result<i32> {
            bail!("test error")
        }

        assert_eq!(test_function().unwrap(), 42);
        assert!(test_function_error().is_err());
    }

    #[test]
    fn test_error_downcast() {
        fn returns_rustowl_error() -> Result<()> {
            Err(RustOwlError::Cache("cache error".to_string()).into())
        }

        let err = returns_rustowl_error().unwrap_err();
        let downcasted = err.downcast::<RustOwlError>().unwrap();
        match downcasted {
            RustOwlError::Cache(msg) => assert_eq!(msg, "cache error"),
            _ => panic!("Expected Cache variant"),
        }
    }
}
