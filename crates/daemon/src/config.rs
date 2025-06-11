//! Configuration management for Gate daemon

use crate::Result;
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

    /// Upstream provider configuration
    pub upstream: UpstreamConfig,

    /// Data directory for storage
    pub data_dir: PathBuf,
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

/// Upstream inference provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    /// Default upstream provider URL
    pub default_url: String,

    /// Upstream request timeout in seconds
    pub timeout_secs: u64,

    /// API key for upstream provider
    pub api_key: Option<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            http: HttpConfig::default(),
            p2p: P2PConfig::default(),
            upstream: UpstreamConfig::default(),
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("gate"),
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8080".parse().unwrap(),
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

impl Default for UpstreamConfig {
    fn default() -> Self {
        Self {
            default_url: "https://api.openai.com/v1".to_string(),
            timeout_secs: 60,
            api_key: None,
        }
    }
}

impl DaemonConfig {
    /// Load configuration from file
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration file cannot be read or parsed
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::from(path.as_ref()))
            .add_source(config::Environment::with_prefix("GATE"))
            .build()?;

        Ok(settings.try_deserialize()?)
    }

    /// Load configuration with defaults and environment variables
    ///
    /// # Errors
    ///
    /// Returns an error if environment variables cannot be parsed
    pub fn from_env() -> Result<Self> {
        let defaults = Self::default();

        let settings = config::Config::builder()
            // Set default values
            .set_default("http.bind_addr", defaults.http.bind_addr.to_string())?
            .set_default("http.cors_enabled", defaults.http.cors_enabled)?
            .set_default("http.timeout_secs", defaults.http.timeout_secs)?
            .set_default("p2p.discovery_enabled", defaults.p2p.discovery_enabled)?
            .set_default("p2p.bootstrap_peers", defaults.p2p.bootstrap_peers)?
            .set_default("upstream.default_url", defaults.upstream.default_url)?
            .set_default("upstream.timeout_secs", defaults.upstream.timeout_secs)?
            .set_default("data_dir", defaults.data_dir.to_string_lossy().to_string())?
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}
