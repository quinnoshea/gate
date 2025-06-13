use thiserror::Error;

pub type Result<T> = std::result::Result<T, RelayError>;

#[derive(Error, Debug)]
pub enum RelayError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),

    #[error("P2P error: {0}")]
    P2P(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("DNS error: {0}")]
    Dns(String),

    #[error("ACME error: {0}")]
    Acme(String),

    #[error("SNI extraction failed: {0}")]
    SniExtraction(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("No idle connection available: {0}")]
    NoIdleConnection(String),

    #[error("Invalid domain: {domain}")]
    InvalidDomain { domain: String },

    #[error("Certificate error: {0}")]
    Certificate(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Timeout: {operation}")]
    Timeout { operation: String },

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Invalid peer address: {0}")]
    InvalidPeerAddress(String),
}

impl From<String> for RelayError {
    fn from(s: String) -> Self {
        RelayError::InvalidPeerAddress(s)
    }
}
