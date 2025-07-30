//! Authentication service for coordinating auth operations

use crate::error::HttpError;
use crate::middleware::auth::AuthenticatedUser;
use crate::services::JwtService;
use crate::types::{AuthCompleteResponse, RegisterCompleteResponse};
use chrono::Utc;
use gate_core::{StateBackend, StoredCredential, User, WebAuthnBackend};
use std::sync::Arc;
use webauthn_rs::prelude::*;

/// Authentication service that coordinates JWT and WebAuthn operations
pub struct AuthService {
    jwt_service: Arc<JwtService>,
    state_backend: Arc<dyn StateBackend>,
    webauthn_backend: Arc<dyn WebAuthnBackend>,
}

impl AuthService {
    /// Create a new authentication service
    pub fn new(
        jwt_service: Arc<JwtService>,
        state_backend: Arc<dyn StateBackend>,
        webauthn_backend: Arc<dyn WebAuthnBackend>,
    ) -> Self {
        Self {
            jwt_service,
            state_backend,
            webauthn_backend,
        }
    }

    /// Complete user registration and return auth response
    pub async fn complete_registration(
        &self,
        user: User,
        credential_id: String,
        device_name: Option<String>,
        passkey: Passkey,
    ) -> Result<RegisterCompleteResponse, HttpError> {
        // Store user in database first
        self.state_backend
            .create_user(&user)
            .await
            .map_err(|e| HttpError::InternalServerError(format!("Failed to create user: {e}")))?;

        // Serialize the passkey for storage
        let passkey_data = serde_json::to_vec(&passkey).map_err(|e| {
            HttpError::InternalServerError(format!("Failed to serialize passkey: {e}"))
        })?;

        // Now store the WebAuthn credential
        let stored_credential = StoredCredential {
            credential_id: credential_id.clone(),
            user_id: user.id.clone(),
            public_key: passkey_data,
            aaguid: None,
            counter: 0, // Initial counter value
            created_at: Utc::now(),
            last_used_at: None,
            device_name,
        };

        self.webauthn_backend
            .store_webauthn_credential(&stored_credential)
            .await
            .map_err(|e| {
                HttpError::InternalServerError(format!("Failed to store credential: {e}"))
            })?;

        // Generate JWT token
        let token = self
            .jwt_service
            .generate_token(&user.id, user.name.as_deref())?;

        Ok(RegisterCompleteResponse {
            user_id: user.id.clone(),
            name: user.name.unwrap_or_else(|| "User".to_string()),
            credential_id,
            token,
        })
    }

    /// Complete user authentication and return auth response
    pub async fn complete_authentication(
        &self,
        credential_id: String,
        counter: u32,
    ) -> Result<AuthCompleteResponse, HttpError> {
        // Get user by credential ID
        let user = self
            .state_backend
            .get_user_by_id(&credential_id)
            .await
            .map_err(|e| HttpError::InternalServerError(format!("Failed to get user: {e}")))?
            .ok_or_else(|| HttpError::NotFound("User not found".to_string()))?;

        // Update credential counter
        self.webauthn_backend
            .update_credential_counter(&credential_id, counter)
            .await
            .map_err(|e| {
                HttpError::InternalServerError(format!("Failed to update credential: {e}"))
            })?;

        // Generate JWT token
        let token = self
            .jwt_service
            .generate_token(&user.id, user.name.as_deref())?;

        Ok(AuthCompleteResponse {
            user_id: user.id,
            name: user.name.unwrap_or_else(|| "User".to_string()),
            token,
        })
    }

    /// Validate a JWT token and return authenticated user
    pub fn validate_token(&self, token: &str) -> Result<AuthenticatedUser, HttpError> {
        let claims = self.jwt_service.validate_token(token)?;

        // TODO: invalidation check

        Ok(AuthenticatedUser {
            id: claims.sub,
            name: claims.name,
            email: None,
            metadata: serde_json::json!({
                "auth_method": "webauthn",
                "issued_at": claims.iat,
                "expires_at": claims.exp,
            }),
        })
    }

    /// Extract and validate token from Authorization header
    pub fn authenticate_from_header(
        &self,
        auth_header: &str,
    ) -> Result<AuthenticatedUser, HttpError> {
        let token = self.jwt_service.extract_bearer_token(auth_header)?;
        self.validate_token(token)
    }
}
