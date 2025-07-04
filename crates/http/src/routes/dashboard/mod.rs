//! Dashboard API routes

use utoipa_axum::router::OpenApiRouter;

use crate::AppState;

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
        + AsRef<std::sync::Arc<crate::services::AuthService>>
        + AsRef<std::sync::Arc<crate::services::WebAuthnService>>
        + AsRef<std::sync::Arc<crate::services::JwtService>>,
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
