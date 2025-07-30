//! Authentication configuration

#[cfg(not(target_arch = "wasm32"))]
use chrono::Duration;
use serde::{Deserialize, Serialize};

/// JWT configuration that can be serialized
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JwtConfigData {
    /// Secret key for signing tokens
    pub secret: String,
    /// Token expiration duration in seconds
    pub expiration_seconds: i64,
    /// Token issuer
    pub issuer: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl From<JwtConfigData> for crate::services::jwt::JwtConfig {
    fn from(data: JwtConfigData) -> Self {
        Self {
            secret: data.secret,
            expiration: Duration::seconds(data.expiration_seconds),
            issuer: data.issuer,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<crate::services::jwt::JwtConfig> for JwtConfigData {
    fn from(config: crate::services::jwt::JwtConfig) -> Self {
        Self {
            secret: config.secret,
            expiration_seconds: config.expiration.num_seconds(),
            issuer: config.issuer,
        }
    }
}

/// Complete authentication configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT configuration
    pub jwt: JwtConfigData,
    /// Session configuration
    pub session: SessionConfig,
}

/// Session configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Session timeout in seconds
    pub timeout_seconds: u64,
    /// Session cleanup interval in seconds
    pub cleanup_interval_seconds: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 86400,         // 24 hours
            cleanup_interval_seconds: 3600, // 1 hour
        }
    }
}
