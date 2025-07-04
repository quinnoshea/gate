//! Custom hooks for the application

pub mod use_auth_callback;
pub mod use_webauthn;

pub use use_webauthn::{WebAuthnState, use_webauthn};
