//! Authentication middleware

use crate::error::HttpError;
use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Request},
    http::request::Parts,
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};

/// Represents an authenticated user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub metadata: serde_json::Value,
}

/// Trait for authentication providers
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Authenticate a request and return user information if successful
    async fn authenticate(&self, parts: &Parts) -> Result<AuthenticatedUser, HttpError>;

    /// Check if authentication should be skipped for a given path
    fn should_skip_auth(&self, path: &str) -> bool {
        // Default implementation - skip auth for health checks and docs
        path == "/health" || path.starts_with("/swagger-ui") || path == "/"
    }
}

/// Middleware function for authentication
pub async fn auth_middleware<T>(
    axum::extract::State(app_state): axum::extract::State<crate::AppState<T>>,
    req: Request,
    next: Next,
) -> Result<Response, HttpError>
where
    crate::AppState<T>: AuthProvider,
    T: Clone + Send + Sync + 'static,
{
    let path = req.uri().path();

    // Skip authentication for certain paths
    if app_state.should_skip_auth(path) {
        return Ok(next.run(req).await);
    }

    // Extract request parts for authentication
    let (mut parts, body) = req.into_parts();

    // Authenticate the request
    match app_state.authenticate(&parts).await {
        Ok(user) => {
            // Insert authenticated user into request extensions
            parts.extensions.insert(user);

            // Reconstruct request
            let req = Request::from_parts(parts, body);
            Ok(next.run(req).await)
        }
        Err(e) => Err(e),
    }
}

/// Service-based authentication provider
#[cfg(not(target_arch = "wasm32"))]
pub struct ServiceAuthProvider {
    auth_service: std::sync::Arc<crate::services::AuthService>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ServiceAuthProvider {
    /// Create a new service-based auth provider
    pub fn new(auth_service: std::sync::Arc<crate::services::AuthService>) -> Self {
        Self { auth_service }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl AuthProvider for ServiceAuthProvider {
    async fn authenticate(&self, parts: &Parts) -> Result<AuthenticatedUser, HttpError> {
        // Extract Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                HttpError::AuthenticationFailed("Missing authorization header".to_string())
            })?;

        // Use auth service to authenticate
        self.auth_service.authenticate_from_header(auth_header)
    }

    fn should_skip_auth(&self, path: &str) -> bool {
        // Allow access to WebAuthn endpoints without authentication
        path.starts_with("/auth/webauthn/") ||
        // Allow access to bootstrap endpoints without authentication
        path.starts_with("/auth/bootstrap/") ||
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

// Implement AuthProvider for AppState when T has AuthService
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T> AuthProvider for crate::AppState<T>
where
    T: AsRef<std::sync::Arc<crate::services::AuthService>> + Send + Sync,
{
    async fn authenticate(&self, parts: &Parts) -> Result<AuthenticatedUser, HttpError> {
        let auth_service: &std::sync::Arc<crate::services::AuthService> =
            self.data.as_ref().as_ref();
        let service_provider = ServiceAuthProvider::new(auth_service.clone());
        service_provider.authenticate(parts).await
    }

    fn should_skip_auth(&self, path: &str) -> bool {
        let auth_service: &std::sync::Arc<crate::services::AuthService> =
            self.data.as_ref().as_ref();
        let service_provider = ServiceAuthProvider::new(auth_service.clone());
        service_provider.should_skip_auth(path)
    }
}

// Implement FromRequestParts for AuthenticatedUser to allow extraction in handlers
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| HttpError::AuthenticationFailed("User not authenticated".to_string()))
    }
}
