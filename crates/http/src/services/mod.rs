//! Service layer for business logic

#[cfg(not(target_arch = "wasm32"))]
pub mod auth;
#[cfg(not(target_arch = "wasm32"))]
pub mod jwt;
#[cfg(not(target_arch = "wasm32"))]
pub mod webauthn;

#[cfg(not(target_arch = "wasm32"))]
pub use auth::AuthService;
#[cfg(not(target_arch = "wasm32"))]
pub use jwt::{Claims, JwtConfig, JwtService};
#[cfg(not(target_arch = "wasm32"))]
pub use webauthn::WebAuthnService;
