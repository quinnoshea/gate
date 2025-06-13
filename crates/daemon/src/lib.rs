//! Gate daemon for P2P AI inference network

pub mod certs;
pub mod config;
pub mod daemon;
pub mod http;
pub mod service;
pub mod tls;
pub mod tls_bridge;
pub mod upstream;

#[cfg(test)]
mod test_iroh;

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
pub use certs::{CertificateManager, CertificateInfo, CertificateType, TlsCertData};
pub use tls::TlsHandler;
pub use tls_bridge::TlsBridge;

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
    Bind(#[from] iroh::endpoint::BindError),
}
