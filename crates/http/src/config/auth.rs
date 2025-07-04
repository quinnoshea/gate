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

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            jwt: crate::services::jwt::JwtConfig::default().into(),
            #[cfg(target_arch = "wasm32")]
            jwt: JwtConfigData {
                secret: "your-secret-key-change-this-in-production".to_string(),
                expiration_seconds: 86400, // 24 hours
                issuer: "gate-server".to_string(),
            },
            session: SessionConfig::default(),
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 86400,         // 24 hours
            cleanup_interval_seconds: 3600, // 1 hour
        }
    }
}

impl AuthConfig {
    /// Create auth config from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // JWT configuration
        if let Ok(secret) = std::env::var("JWT_SECRET") {
            config.jwt.secret = secret;
        }
        if let Ok(hours) = std::env::var("JWT_EXPIRATION_HOURS")
            && let Ok(hours) = hours.parse::<i64>()
        {
            config.jwt.expiration_seconds = hours * 3600;
        }
        if let Ok(issuer) = std::env::var("JWT_ISSUER") {
            config.jwt.issuer = issuer;
        }

        // Session configuration
        if let Ok(timeout) = std::env::var("SESSION_TIMEOUT_SECONDS")
            && let Ok(timeout) = timeout.parse::<u64>()
        {
            config.session.timeout_seconds = timeout;
        }

        config
    }

    /// Create development configuration
    pub fn development() -> Self {
        Self {
            jwt: JwtConfigData {
                secret: "development-secret-key".to_string(),
                expiration_seconds: 24 * 3600,
                issuer: "gate-dev".to_string(),
            },
            session: SessionConfig {
                timeout_seconds: 86400,
                cleanup_interval_seconds: 3600,
            },
        }
    }

    /// Create production configuration
    pub fn production() -> Self {
        let config = Self::from_env();

        // Validate production requirements
        if config.jwt.secret == "your-secret-key-change-this-in-production" {
            panic!("JWT_SECRET must be set in production");
        }

        config
    }
}
