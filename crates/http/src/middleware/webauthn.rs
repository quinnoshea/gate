//! WebAuthn authentication middleware and types

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use webauthn_rs::{Webauthn, WebauthnBuilder};

/// WebAuthn configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnConfig {
    /// Relying Party ID (domain name)
    pub rp_id: String,
    /// Relying Party Name (display name)
    pub rp_name: String,
    /// Relying Party origin (https://example.com)
    pub rp_origin: String,
    /// Additional allowed origins
    pub allowed_origins: Vec<String>,
    /// Allow relay origins automatically
    pub allow_tlsforward_origins: bool,
    /// Allow subdomains of configured origins
    pub allow_subdomains: bool,
    /// Require user verification
    pub require_user_verification: bool,
    /// Session timeout in seconds
    pub session_timeout_seconds: u64,
}

/// Session data for ongoing WebAuthn operations
#[derive(Debug, Clone)]
pub struct WebAuthnSession {
    pub user_name: Option<String>,
    pub registration_state: Option<webauthn_rs::prelude::PasskeyRegistration>,
    pub authentication_state: Option<webauthn_rs::prelude::PasskeyAuthentication>,
    pub created_at: DateTime<Utc>,
}

/// WebAuthn state manager
pub struct WebAuthnState {
    webauthn: Arc<RwLock<Webauthn>>,
    sessions: Arc<RwLock<std::collections::HashMap<String, WebAuthnSession>>>,
    config: Arc<RwLock<WebAuthnConfig>>,
}

impl WebAuthnState {
    /// Create a new WebAuthn state manager
    pub fn new(config: WebAuthnConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let origin_url = webauthn_rs::prelude::Url::parse(&config.rp_origin)?;
        let mut builder = WebauthnBuilder::new(&config.rp_id, &origin_url)?;

        // Set the RP name
        builder = builder.rp_name(&config.rp_name);

        // Add additional allowed origins
        for origin_str in &config.allowed_origins {
            if let Ok(origin) = webauthn_rs::prelude::Url::parse(origin_str) {
                builder = builder.append_allowed_origin(&origin);
            }
        }

        // Configure subdomain support
        if config.allow_subdomains {
            builder = builder.allow_subdomains(true);
        }

        let webauthn = Arc::new(RwLock::new(builder.build()?));

        Ok(Self {
            webauthn,
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            config: Arc::new(RwLock::new(config)),
        })
    }

    /// Generate a new session ID
    pub fn generate_session_id() -> String {
        let random_bytes: [u8; 32] = rand::random();
        URL_SAFE_NO_PAD.encode(random_bytes)
    }

    /// Store a session
    pub async fn store_session(&self, session_id: String, session: WebAuthnSession) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, session);
    }

    /// Get a session
    pub async fn get_session(&self, session_id: &str) -> Option<WebAuthnSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Remove a session
    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let now = Utc::now();
        let config = self.config.read().await;
        let timeout = chrono::Duration::seconds(config.session_timeout_seconds as i64);

        sessions.retain(|_, session| now - session.created_at < timeout);
    }

    /// Get the WebAuthn instance
    pub fn webauthn(&self) -> Arc<RwLock<Webauthn>> {
        self.webauthn.clone()
    }

    /// Add a new allowed origin dynamically
    pub async fn add_allowed_origin(
        &self,
        origin: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = self.config.write().await;

        // Check if origin already exists
        if config.allowed_origins.contains(&origin) {
            return Ok(());
        }

        // Add the new origin to config
        config.allowed_origins.push(origin.clone());

        // Rebuild WebAuthn instance with new origins
        let origin_url = webauthn_rs::prelude::Url::parse(&config.rp_origin)?;
        let mut builder = WebauthnBuilder::new(&config.rp_id, &origin_url)?;

        // Set the RP name
        builder = builder.rp_name(&config.rp_name);

        // Add all allowed origins
        for origin_str in &config.allowed_origins {
            if let Ok(origin) = webauthn_rs::prelude::Url::parse(origin_str) {
                builder = builder.append_allowed_origin(&origin);
            }
        }

        // Configure subdomain support
        if config.allow_subdomains {
            builder = builder.allow_subdomains(true);
        }

        let new_webauthn = builder.build()?;
        let mut webauthn = self.webauthn.write().await;
        *webauthn = new_webauthn;

        tracing::info!("Added allowed origin for WebAuthn: {}", origin);

        Ok(())
    }
}
