//! Gate daemon for P2P AI inference network

pub mod config;
pub mod daemon;
pub mod http;
pub mod letsencrypt;
pub mod selfsigned;
pub mod tls;
pub mod tls_bridge;
pub mod upstream;

pub use config::{
    DaemonConfig, HttpConfig, LetsEncryptConfig, P2PConfig, TlsConfig, UpstreamConfig,
};
pub use daemon::GateDaemon;
pub use upstream::{InferenceRequest, UpstreamResponse};

/// Result type for daemon operations
pub type Result<T> = std::result::Result<T, DaemonError>;

/// Daemon error types
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("Configuration error: {0}")]
    Config(#[from] ::config::ConfigError),

    #[error("P2P error: {0}")]
    P2P(#[from] hellas_gate_p2p::P2PError),

    #[error("HTTP server error: {0}")]
    Http(String),

    #[error("Upstream provider error: {0}")]
    Upstream(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}
