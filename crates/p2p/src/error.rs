//! Error types for P2P networking

use thiserror::Error;

#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Connection error: {0}")]
    Connection(#[from] iroh::endpoint::ConnectionError),

    #[error("Iroh error: {0}")]
    Iroh(#[from] anyhow::Error),

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}
