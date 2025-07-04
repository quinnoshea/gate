//! WebAuthn authentication provider

use crate::{
    error::HttpError,
    middleware::auth::{AuthProvider, AuthenticatedUser},
    services::AuthService,
};
use async_trait::async_trait;
use axum::http::request::Parts;
use std::sync::Arc;

/// WebAuthn authentication provider that validates JWT tokens
pub struct WebAuthnAuthProvider {
    auth_service: Arc<AuthService>,
}

impl WebAuthnAuthProvider {
    /// Create a new WebAuthn auth provider
    pub fn new(auth_service: Arc<AuthService>) -> Self {
        Self { auth_service }
    }
}

#[async_trait]
impl AuthProvider for WebAuthnAuthProvider {
    async fn authenticate(&self, parts: &Parts) -> Result<AuthenticatedUser, HttpError> {
        // Extract Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| HttpError::AuthenticationFailed("Missing authorization header".to_string()))?;

        // Use auth service to authenticate
        self.auth_service.authenticate_from_header(auth_header)
    }

    fn should_skip_auth(&self, path: &str) -> bool {
        // Allow access to WebAuthn endpoints without authentication
        path.starts_with("/auth/webauthn/") ||
        // Default paths that should be public
        path == "/health" || 
        path.starts_with("/swagger-ui") || 
        path == "/" ||
        // Allow access to frontend assets
        path.ends_with(".js") ||
        path.ends_with(".wasm") ||
        path.ends_with(".html") ||
        path.ends_with(".css")
    }
}

