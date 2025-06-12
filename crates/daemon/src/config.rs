//! Configuration structures for Gate daemon

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Main daemon configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// HTTP server configuration
    pub http: HttpConfig,

    /// P2P networking configuration
    pub p2p: P2PConfig,

    /// TLS server configuration
    pub tls: TlsConfig,

    /// Upstream provider configuration
    pub upstream: UpstreamConfig,
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Address to bind HTTP server
    pub bind_addr: SocketAddr,

    /// Enable CORS for web interface
    pub cors_enabled: bool,

    /// Request timeout in seconds
    pub timeout_secs: u64,
}

/// P2P networking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PConfig {
    /// Optional path to identity key file
    pub identity_file: Option<PathBuf>,

    /// Known peers to connect to at startup
    pub bootstrap_peers: Vec<String>,

    /// Enable discovery
    pub discovery_enabled: bool,

    /// Port to bind P2P endpoint to (0 for random)
    pub port: u16,
}

/// TLS server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Address to bind TLS server
    pub bind_addr: SocketAddr,

    /// Enable TLS server
    pub enabled: bool,

    /// LetsEncrypt configuration (optional)
    pub letsencrypt: Option<LetsEncryptConfig>,
}

/// LetsEncrypt ACME configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetsEncryptConfig {
    /// Email address for ACME account registration
    pub email: String,

    /// Use staging environment (for testing)
    pub staging: bool,

    /// Domains to request certificates for
    pub domains: Vec<String>,

    /// Enable automatic certificate renewal
    pub auto_renew: bool,
}

/// Upstream inference provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    /// Default upstream provider URL
    pub default_url: String,

    /// Upstream request timeout in seconds
    pub timeout_secs: u64,

    /// API key for upstream provider
    pub api_key: Option<String>,

    /// Model to use for testing upstream connection
    pub test_model: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            http: HttpConfig::default(),
            p2p: P2PConfig::default(),
            tls: TlsConfig::default(),
            upstream: UpstreamConfig::default(),
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:38080".parse().unwrap(),
            cors_enabled: true,
            timeout_secs: 30,
        }
    }
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            identity_file: None,
            bootstrap_peers: vec![],
            discovery_enabled: true,
            port: 31145, // Default P2P port
        }
    }
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8443".parse().unwrap(),
            enabled: true,
            letsencrypt: None,
        }
    }
}

impl Default for LetsEncryptConfig {
    fn default() -> Self {
        Self {
            email: "accounts@hellas.ai".to_string(),
            staging: true,
            domains: vec![],
            auto_renew: true,
        }
    }
}

impl Default for UpstreamConfig {
    fn default() -> Self {
        Self {
            default_url: "https://api.openai.com/v1".to_string(),
            timeout_secs: 60,
            api_key: None,
            test_model: "gemma-3-1b-it-qat".to_string(),
        }
    }
}
