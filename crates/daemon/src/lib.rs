//! Gate daemon for P2P AI inference network

pub mod certs;
pub mod config;
pub mod daemon;
pub mod http;
pub mod service;
pub mod tls;
pub mod upstream;

// Core daemon exports
pub use daemon::GateDaemon;
pub use service::DaemonServiceImpl;

// Configuration exports
pub use config::{
    DaemonConfig, HttpConfig, LetsEncryptConfig, P2PConfig, TlsConfig, UpstreamConfig,
};

// Upstream integration
pub use upstream::{InferenceRequest, UpstreamResponse};

// TLS functionality (grouped for clarity)
pub use certs::{CertificateInfo, CertificateManager, CertificateType, TlsCertData};
pub use tls::TlsHandler;

// Core error handling
pub use hellas_gate_core::{CoreError, ErrorContext};

// Daemon-specific error handling
/// Result type for daemon operations
pub type Result<T> = std::result::Result<T, DaemonError>;

/// Daemon error types
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("Configuration error: {0}")]
    Config(#[from] ::config::ConfigError),

    #[error("Configuration error: {0}")]
    ConfigString(String),

    #[error("P2P error: {0}")]
    P2P(String),

    #[error("TLS termination failed: {0}")]
    TlsTermination(#[from] tokio_rustls::rustls::Error),

    #[error("Stream handling failed: {0}")]
    StreamHandling(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("RPC client error: {0}")]
    Client(String),

    #[error("HTTP server error: {0}")]
    Http(String),

    #[error("Upstream provider error: {0}")]
    Upstream(String),

    #[error("Certificate error: {0}")]
    Certificate(String),

    #[error("ACME error: {0}")]
    Acme(#[from] instant_acme::Error),

    #[error("Certificate generation error: {0}")]
    Rcgen(#[from] rcgen::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),

    #[error("Disconnected: {0}")]
    Disconnected(#[from] n0_watcher::Disconnected),

    #[error("Bind error: {0}")]
    Bind(String),

    #[error("Core error: {0}")]
    Core(#[from] CoreError),
}

impl DaemonError {
    /// Create a certificate error with context
    pub fn certificate_error(message: impl Into<String>) -> Self {
        Self::Certificate(message.into())
    }

    /// Create a P2P error with context
    pub fn p2p_error(message: impl Into<String>) -> Self {
        Self::P2P(message.into())
    }

    /// Create an HTTP error with context
    pub fn http_error(message: impl Into<String>) -> Self {
        Self::Http(message.into())
    }

    /// Create an upstream error with context
    pub fn upstream_error(message: impl Into<String>) -> Self {
        Self::Upstream(message.into())
    }
}

/// Extension trait for adding context to IO operations specifically for daemon errors
pub trait DaemonErrorContext<T> {
    /// Add certificate-related context to an error
    fn with_certificate_context(self, operation: &str) -> Result<T>;

    /// Add P2P-related context to an error
    fn with_p2p_context(self, operation: &str) -> Result<T>;

    /// Add HTTP-related context to an error
    fn with_http_context(self, operation: &str) -> Result<T>;
}

impl<T> DaemonErrorContext<T> for std::result::Result<T, std::io::Error> {
    fn with_certificate_context(self, operation: &str) -> Result<T> {
        self.map_err(|e| DaemonError::certificate_error(format!("{}: {}", operation, e)))
    }

    fn with_p2p_context(self, operation: &str) -> Result<T> {
        self.map_err(|e| DaemonError::p2p_error(format!("{}: {}", operation, e)))
    }

    fn with_http_context(self, operation: &str) -> Result<T> {
        self.map_err(|e| DaemonError::http_error(format!("{}: {}", operation, e)))
    }
}

impl<T> DaemonErrorContext<T> for std::result::Result<T, serde_json::Error> {
    fn with_certificate_context(self, operation: &str) -> Result<T> {
        self.map_err(|e| DaemonError::certificate_error(format!("{}: {}", operation, e)))
    }

    fn with_p2p_context(self, operation: &str) -> Result<T> {
        self.map_err(|e| DaemonError::p2p_error(format!("{}: {}", operation, e)))
    }

    fn with_http_context(self, operation: &str) -> Result<T> {
        self.map_err(|e| DaemonError::http_error(format!("{}: {}", operation, e)))
    }
}
