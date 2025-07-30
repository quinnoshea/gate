//! Dashboard API routes

use crate::AppState;
use std::sync::Arc;
use utoipa_axum::router::OpenApiRouter;

pub mod api_keys;
#[cfg(not(target_arch = "wasm32"))]
pub mod auth;
pub mod user;

#[cfg(not(target_arch = "wasm32"))]
pub fn add_routes<T>(router: OpenApiRouter<AppState<T>>) -> OpenApiRouter<AppState<T>>
where
    T: Send
        + Sync
        + Clone
        + 'static
        + AsRef<Arc<crate::services::AuthService>>
        + AsRef<Option<Arc<crate::services::WebAuthnService>>>
        + AsRef<Arc<crate::services::JwtService>>,
{
    router
        .merge(api_keys::router::<T>())
        .merge(auth::router::<T>())
        .merge(user::router::<T>())
}

#[cfg(target_arch = "wasm32")]
pub fn add_routes<T>(router: OpenApiRouter<AppState<T>>) -> OpenApiRouter<AppState<T>>
where
    T: Send + Sync + Clone + 'static,
{
    router
        .merge(api_keys::router::<T>())
        .merge(user::router::<T>())
}
