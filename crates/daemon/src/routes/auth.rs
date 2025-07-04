//! Custom authentication routes with registration control

use crate::bootstrap::BootstrapTokenManager;
use crate::config::Settings;
use axum::{extract::State, response::Json};
use chrono::Utc;
use gate_core::{BootstrapTokenValidator, types::User};
use gate_http::{
    error::HttpError,
    middleware::auth::AuthenticatedUser,
    services::{AuthService, WebAuthnService},
    state::AppState,
    types::{RegisterCompleteRequest, RegisterCompleteResponse},
};
use std::sync::Arc;
use tracing::{info, instrument, warn};
use utoipa_axum::{router::OpenApiRouter, routes};

/// Custom registration complete handler with bootstrap and registration control
#[utoipa::path(
    post,
    path = "/auth/webauthn/register/complete-custom",
    request_body = RegisterCompleteRequest,
    responses(
        (status = 200, description = "Registration completed", body = RegisterCompleteResponse),
        (status = 400, description = "Bad request"),
        (status = 403, description = "Registration not allowed"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "authentication"
)]
#[instrument(
    name = "webauthn_register_complete_with_control",
    skip(app_state, request),
    fields(
        session_id = %request.session_id,
        device_name = ?request.device_name,
        has_bootstrap_token = request.bootstrap_token.is_some()
    )
)]
pub async fn register_complete<T>(
    State(app_state): State<AppState<T>>,
    Json(request): Json<RegisterCompleteRequest>,
) -> Result<Json<RegisterCompleteResponse>, HttpError>
where
    T: Clone
        + Send
        + Sync
        + 'static
        + AsRef<Arc<WebAuthnService>>
        + AsRef<Arc<AuthService>>
        + AsRef<Arc<Settings>>
        + AsRef<Arc<BootstrapTokenManager>>,
{
    let webauthn_service: &Arc<WebAuthnService> = app_state.data.as_ref().as_ref();
    let auth_service: &Arc<AuthService> = app_state.data.as_ref().as_ref();
    let settings: &Arc<Settings> = app_state.data.as_ref().as_ref();
    let bootstrap_manager: &Arc<BootstrapTokenManager> = app_state.data.as_ref().as_ref();

    // Check if this is the first user (bootstrap needed)
    let needs_bootstrap = bootstrap_manager.needs_bootstrap().await.map_err(|e| {
        HttpError::InternalServerError(format!("Failed to check bootstrap status: {e}"))
    })?;

    let role = if needs_bootstrap {
        // First user - require bootstrap token
        match &request.bootstrap_token {
            Some(token) => {
                let valid = bootstrap_manager.validate_token(token).await.map_err(|e| {
                    HttpError::InternalServerError(format!("Failed to validate token: {e}"))
                })?;

                if !valid {
                    return Err(HttpError::AuthorizationFailed(
                        "Invalid bootstrap token".to_string(),
                    ));
                }

                info!("Valid bootstrap token provided for first user registration");
                settings.auth.registration.bootstrap_admin_role.clone()
            }
            None => {
                return Err(HttpError::AuthorizationFailed(
                    "Bootstrap token required for first user registration".to_string(),
                ));
            }
        }
    } else {
        // Not first user - check if open registration is allowed
        if !settings.auth.registration.allow_open_registration {
            warn!("Registration attempt blocked - open registration is disabled");
            return Err(HttpError::AuthorizationFailed(
                "Registration is closed. Please contact an administrator.".to_string(),
            ));
        }

        // Warn if bootstrap token was provided but not needed
        if request.bootstrap_token.is_some() {
            warn!("Bootstrap token provided but system is already bootstrapped");
        }

        settings.auth.registration.default_user_role.clone()
    };

    // Complete the WebAuthn registration
    let device_name = request.device_name.clone();
    let (passkey, credential_id, user_name) = webauthn_service
        .complete_registration(request.session_id, request.credential, device_name.clone())
        .await?;

    // Create user with appropriate role
    let user = User {
        id: credential_id.clone(),
        name: Some(user_name.clone()),
        role,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metadata: std::collections::HashMap::new(),
    };

    let response = auth_service
        .complete_registration(user.clone(), credential_id.clone(), device_name, passkey)
        .await?;

    // If this was bootstrap, mark the token as used
    if needs_bootstrap {
        bootstrap_manager.mark_token_as_used().await.map_err(|e| {
            HttpError::InternalServerError(format!("Failed to mark bootstrap token as used: {e}"))
        })?;
        info!(
            "Bootstrap completed - admin user created: {} ({}) with role: {}",
            user_name, credential_id, user.role
        );
    } else {
        info!(
            "User registered: {} ({}) with role: {}",
            user_name, credential_id, user.role
        );
    }

    Ok(Json(response))
}

/// Check bootstrap status
#[utoipa::path(
    get,
    path = "/auth/bootstrap/status",
    responses(
        (status = 200, description = "Bootstrap status", body = serde_json::Value),
        (status = 500, description = "Internal server error"),
    ),
    tag = "authentication"
)]
#[instrument(name = "get_bootstrap_status", skip(app_state))]
pub async fn get_bootstrap_status<T>(
    State(app_state): State<AppState<T>>,
) -> Result<Json<serde_json::Value>, HttpError>
where
    T: Clone + Send + Sync + 'static + AsRef<Arc<BootstrapTokenManager>>,
{
    let bootstrap_manager: &Arc<BootstrapTokenManager> = app_state.data.as_ref().as_ref();

    let needs_bootstrap = bootstrap_manager.needs_bootstrap().await.map_err(|e| {
        HttpError::InternalServerError(format!("Failed to check bootstrap status: {e}"))
    })?;

    let is_complete = bootstrap_manager.is_bootstrap_complete().await;

    Ok(Json(serde_json::json!({
        "needs_bootstrap": needs_bootstrap,
        "is_complete": is_complete,
        "message": if needs_bootstrap {
            "System requires initial admin user setup"
        } else {
            "System is bootstrapped"
        }
    })))
}

/// Get bootstrap token for initial admin setup
#[utoipa::path(
    get,
    path = "/auth/bootstrap/token",
    responses(
        (status = 200, description = "Bootstrap token", body = serde_json::Value),
        (status = 400, description = "Bootstrap already complete"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "authentication"
)]
#[instrument(name = "get_bootstrap_token", skip(app_state))]
pub async fn get_bootstrap_token<T>(
    State(app_state): State<AppState<T>>,
) -> Result<Json<serde_json::Value>, HttpError>
where
    T: Clone + Send + Sync + 'static + AsRef<Arc<BootstrapTokenManager>>,
{
    let bootstrap_manager: &Arc<BootstrapTokenManager> = app_state.data.as_ref().as_ref();

    // Check if bootstrap is needed
    let needs_bootstrap = bootstrap_manager.needs_bootstrap().await.map_err(|e| {
        HttpError::InternalServerError(format!("Failed to check bootstrap status: {e}"))
    })?;

    if !needs_bootstrap {
        return Err(HttpError::BadRequest(
            "Bootstrap has already been completed".to_string(),
        ));
    }

    // Generate token
    let token = bootstrap_manager.generate_token().await.map_err(|e| {
        HttpError::InternalServerError(format!("Failed to generate bootstrap token: {e}"))
    })?;

    Ok(Json(serde_json::json!({
        "token": token,
        "message": "Use this token to register the first admin user"
    })))
}

/// Get current user information
#[utoipa::path(
    get,
    path = "/api/auth/me",
    operation_id = "get_current_user",
    description = "Get current user information",
    responses(
        (status = 200, description = "User information", body = serde_json::Value),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("BearerAuth" = [])
    )
)]
async fn get_current_user<T>(
    State(app_state): State<AppState<T>>,
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, HttpError>
where
    T: AsRef<Arc<WebAuthnService>>
        + AsRef<Arc<AuthService>>
        + AsRef<Arc<Settings>>
        + AsRef<Arc<BootstrapTokenManager>>,
{
    // Get user from database
    let user_data = app_state
        .state_backend
        .get_user(&user.id)
        .await
        .map_err(|e| HttpError::InternalServerError(format!("Failed to get user: {e}")))?
        .ok_or_else(|| HttpError::AuthorizationFailed("User not found".to_string()))?;

    Ok(Json(serde_json::json!({
        "id": user_data.id,
        "name": user_data.name,
        "role": user_data.role,
        "created_at": user_data.created_at,
        "updated_at": user_data.updated_at,
    })))
}

/// Add custom auth routes
pub fn add_routes<T>(router: OpenApiRouter<AppState<T>>) -> OpenApiRouter<AppState<T>>
where
    T: Clone
        + Send
        + Sync
        + 'static
        + AsRef<Arc<WebAuthnService>>
        + AsRef<Arc<AuthService>>
        + AsRef<Arc<Settings>>
        + AsRef<Arc<BootstrapTokenManager>>,
{
    router
        .routes(routes!(register_complete))
        .routes(routes!(get_bootstrap_status))
        .routes(routes!(get_bootstrap_token))
        .routes(routes!(get_current_user))
}
