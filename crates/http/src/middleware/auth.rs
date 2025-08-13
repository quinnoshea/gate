use crate::error::HttpError;
use crate::services::identity::HttpIdentity;
use async_trait::async_trait;
use axum::{extract::Request, http::request::Parts, middleware::Next, response::Response};
use std::sync::Arc;

/// Trait for authentication providers
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Authenticate a request and return identity if successful
    async fn authenticate(&self, parts: &Parts) -> Result<HttpIdentity, HttpError>;

    /// Check if authentication should be skipped for a given path
    fn should_skip_auth(&self, path: &str) -> bool {
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

    if app_state.should_skip_auth(path) {
        return Ok(next.run(req).await);
    }

    let (mut parts, body) = req.into_parts();

    match app_state.authenticate(&parts).await {
        Ok(identity) => {
            parts.extensions.insert(identity);
            let req = Request::from_parts(parts, body);
            Ok(next.run(req).await)
        }
        Err(e) => Err(e),
    }
}

/// Service-based authentication provider
#[cfg(not(target_arch = "wasm32"))]
pub struct ServiceAuthProvider {
    auth_service: Arc<crate::services::AuthService>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ServiceAuthProvider {
    pub fn new(auth_service: Arc<crate::services::AuthService>) -> Self {
        Self { auth_service }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl AuthProvider for ServiceAuthProvider {
    async fn authenticate(&self, parts: &Parts) -> Result<HttpIdentity, HttpError> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                HttpError::AuthenticationFailed("Missing authorization header".to_string())
            })?;

        self.auth_service.authenticate_from_header(auth_header)
    }

    fn should_skip_auth(&self, path: &str) -> bool {
        path.starts_with("/auth/webauthn/")
            || path.starts_with("/auth/bootstrap/")
            || path == "/health"
            || path.starts_with("/swagger-ui")
            || path == "/"
            || path.ends_with(".js")
            || path.ends_with(".wasm")
            || path.ends_with(".html")
            || path.ends_with(".css")
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl<T> AuthProvider for crate::AppState<T>
where
    T: AsRef<Arc<crate::services::AuthService>> + Send + Sync,
{
    async fn authenticate(&self, parts: &Parts) -> Result<HttpIdentity, HttpError> {
        let auth_service: &Arc<crate::services::AuthService> = self.data.as_ref().as_ref();
        let service_provider = ServiceAuthProvider::new(auth_service.clone());
        service_provider.authenticate(parts).await
    }

    fn should_skip_auth(&self, path: &str) -> bool {
        let auth_service: &Arc<crate::services::AuthService> = self.data.as_ref().as_ref();
        let service_provider = ServiceAuthProvider::new(auth_service.clone());
        service_provider.should_skip_auth(path)
    }
}
