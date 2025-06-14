//! Common error handling utilities and conventions

/// Extension trait for adding context to errors consistently across crates
pub trait ErrorContext<T> {
    /// Add operation context to an error result
    fn with_context<F>(self, f: F) -> Result<T, String>
    where
        F: FnOnce() -> String;

    /// Add operation context with a static string
    fn with_context_str(self, context: &'static str) -> Result<T, String>;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: std::fmt::Display,
{
    fn with_context<F>(self, f: F) -> Result<T, String>
    where
        F: FnOnce() -> String,
    {
        match self {
            Ok(val) => Ok(val),
            Err(err) => Err(format!("{}: {}", f(), err)),
        }
    }

    fn with_context_str(self, context: &'static str) -> Result<T, String> {
        self.with_context(|| context.to_string())
    }
}

/// Standard result type for core operations
pub type CoreResult<T> = std::result::Result<T, CoreError>;

/// Core error types that can be shared across crates
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum CoreError {
    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },

    #[error("IO operation failed: {message}")]
    Io { message: String },

    #[error("Serialization error: {message}")]
    Serialization { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl CoreError {
    /// Create an invalid config error
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::InvalidConfig {
            message: message.into(),
        }
    }

    /// Create an IO error
    pub fn io_error(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
        }
    }

    /// Create a serialization error
    pub fn serialization_error(message: impl Into<String>) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        Self::io_error(err.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        Self::serialization_error(err.to_string())
    }
}
