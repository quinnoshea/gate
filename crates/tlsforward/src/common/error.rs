//! Error types for the TLS forward service

use thiserror::Error;

/// Result type alias for TLS forward operations
pub type Result<T> = std::result::Result<T, TlsForwardError>;

/// Errors that can occur in the TLS forward service
#[derive(Debug, Error)]
pub enum TlsForwardError {
    /// I/O error occurred
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// P2P connection error
    #[error("P2P connection error: {0}")]
    P2pConnection(#[from] iroh::endpoint::ConnectionError),

    /// P2P connect error
    #[error("P2P connect error: {0}")]
    P2pConnect(#[from] iroh::endpoint::ConnectError),

    /// P2P write error
    #[error("P2P write error: {0}")]
    P2pWrite(#[from] iroh::endpoint::WriteError),

    /// P2P read error
    #[error("P2P read error: {0}")]
    P2pRead(#[from] iroh::endpoint::ReadError),

    /// Invalid SNI or hostname
    #[error("Invalid SNI: {0}")]
    InvalidSni(String),

    /// Node not found in registry
    #[error("Node not found for domain: {0}")]
    NodeNotFound(String),

    /// TLS/SSL error
    #[error("TLS error: {0}")]
    Tls(String),

    /// Timeout occurred
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(#[from] http::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Registry error
    #[error("Registry error: {0}")]
    Registry(String),

    /// Protocol error
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Cloudflare API error
    #[error("Cloudflare API error: {0}")]
    Cloudflare(String),
}

#[cfg(feature = "server")]
impl From<cloudflare::framework::Error> for TlsForwardError {
    fn from(e: cloudflare::framework::Error) -> Self {
        TlsForwardError::Cloudflare(e.to_string())
    }
}
