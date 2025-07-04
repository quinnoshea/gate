//! Service modules for API and browser interactions

pub mod auth;
pub mod config;
pub mod inference;
pub mod webauthn_browser;

pub use auth::AuthApiService;
pub use config::ConfigApiService;
pub use inference::{ChatMessage, InferenceService, Model, Role};
pub use webauthn_browser::WebAuthnBrowserService;
