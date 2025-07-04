//! Frontend configuration

/// Authentication configuration
pub struct AuthConfig;

impl AuthConfig {
    /// Token refresh interval in milliseconds
    pub const TOKEN_REFRESH_INTERVAL_MS: u32 = 60_000; // 1 minute

    /// Session storage key for auth state
    pub const AUTH_STATE_KEY: &'static str = "auth_state";
}
