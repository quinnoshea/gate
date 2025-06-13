//! Configuration structures for Gate relay server

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Main relay server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// HTTPS server configuration
    pub https: HttpsConfig,

    /// P2P networking configuration
    pub p2p: P2PConfig,

    /// DNS configuration for domain management
    pub dns: DnsConfig,
}

/// HTTPS server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpsConfig {
    /// Address to bind HTTPS server
    pub bind_addr: SocketAddr,

    /// Connection timeout in seconds
    pub timeout_secs: u64,
}

/// P2P networking configuration for relay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PConfig {
    /// Optional path to identity key file
    pub identity_file: Option<PathBuf>,

    /// Port to bind P2P endpoint to (0 for random)
    pub port: u16,

    /// Enable discovery
    pub discovery_enabled: bool,
}

/// DNS configuration for domain management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    /// Base domain for generated subdomains
    pub base_domain: String,

    /// DNS provider configuration
    pub provider: String,

    /// DNS update interval in seconds
    pub update_interval_secs: u64,

    /// A record addresses (IPv4) for domain registration
    pub a_records: Vec<String>,

    /// AAAA record addresses (IPv6) for domain registration
    pub aaaa_records: Vec<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            https: HttpsConfig::default(),
            p2p: P2PConfig::default(),
            dns: DnsConfig::default(),
        }
    }
}

impl Default for HttpsConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8443".parse().unwrap(),
            timeout_secs: 30,
        }
    }
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            identity_file: None,
            port: 41146,
            discovery_enabled: false,
        }
    }
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            base_domain: "private.hellas.ai".to_string(),
            provider: "manual".to_string(), // Manual DNS management by default
            update_interval_secs: 300,      // 5 minutes
            a_records: vec![],
            aaaa_records: vec![],
        }
    }
}
