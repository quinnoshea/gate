//! Server configuration

use config::{Config, ConfigError, Environment, File};
use gate_http::forwarding::UpstreamProvider;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_rp_id() -> String {
    "localhost".to_string()
}

fn default_rp_name() -> String {
    "Gate Self-Hosted".to_string()
}

fn default_rp_origin() -> String {
    "http://localhost:3000".to_string()
}

fn default_session_timeout() -> u64 {
    86400 // 24 hours
}

fn default_timeout() -> u64 {
    30 // 30 seconds
}

fn default_jwt_issuer() -> String {
    "gate-daemon".to_string()
}

fn default_jwt_expiration_hours() -> u64 {
    24 // 24 hours
}

fn default_heartbeat_interval() -> u64 {
    30 // 30 seconds
}

fn default_max_reconnect_attempts() -> u32 {
    10
}

fn default_reconnect_backoff() -> u64 {
    5 // 5 seconds
}

fn default_auto_renew_days() -> u32 {
    30 // 30 days before expiry
}

fn default_tlsforward_max_connections() -> usize {
    1000
}

fn default_user_role() -> String {
    "user".to_string()
}

fn default_admin_roles() -> Vec<String> {
    vec!["admin".to_string()]
}

fn default_bootstrap_role() -> String {
    "admin".to_string()
}

fn default_tlsforward_addresses() -> Vec<String> {
    vec![
        "3dbefb2e3d56c7e32586d9a82167a8a5151f3e0f4b40b7c3d145b9060dde2f14@213.239.212.173:31145"
            .to_string(),
    ]
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// Server settings
    pub server: ServerConfig,
    /// Authentication settings
    pub auth: AuthConfig,
    /// Upstream provider settings (supports multiple)
    pub upstreams: Vec<UpstreamConfig>,
    /// Relay configuration
    pub tlsforward: TlsForwardConfig,
    /// Let's Encrypt configuration
    pub letsencrypt: LetsEncryptConfig,
    /// Local inference configuration
    pub local_inference: Option<LocalInferenceConfig>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to bind to
    #[serde(default = "default_port")]
    pub port: u16,
    /// CORS allowed origins
    #[serde(default)]
    pub cors_origins: Vec<String>,
    /// Prometheus metrics endpoint port (if enabled)
    #[serde(default)]
    pub metrics_port: Option<u16>,
}

/// Authentication configuration
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// WebAuthn configuration
    #[serde(default)]
    pub webauthn: WebAuthnConfig,
    /// JWT configuration
    #[serde(default)]
    pub jwt: JwtConfig,
    /// Registration configuration
    #[serde(default)]
    pub registration: RegistrationConfig,
}

/// Upstream provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    /// Name identifier for this upstream
    pub name: String,
    /// LLM Provider type
    pub provider: UpstreamProvider,
    /// Base URL for the upstream API
    pub base_url: String,
    /// API key for authentication (can be set via env var)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// List of supported models (populated on startup)
    #[serde(default, skip_serializing)]
    pub models: Vec<String>,
}

/// WebAuthn configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthnConfig {
    /// Enable WebAuthn authentication
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Relying Party ID (usually domain name)
    #[serde(default = "default_rp_id")]
    pub rp_id: String,
    /// Relying Party Name (display name)
    #[serde(default = "default_rp_name")]
    pub rp_name: String,
    /// Relying Party Origin (full URL)
    #[serde(default = "default_rp_origin")]
    pub rp_origin: String,
    /// Additional allowed origins
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// Allow relay origins automatically (*.hellas.ai domains)
    #[serde(default = "default_true")]
    pub allow_tlsforward_origins: bool,
    /// Allow subdomains of configured origins
    #[serde(default = "default_true")]
    pub allow_subdomains: bool,
    /// Require user verification
    #[serde(default = "default_true")]
    pub require_user_verification: bool,
    /// Session timeout in seconds
    #[serde(default = "default_session_timeout")]
    pub session_timeout_seconds: u64,
}

/// JWT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    /// JWT issuer
    #[serde(default = "default_jwt_issuer")]
    pub issuer: String,
    /// JWT secret (read from JWT_SECRET env var if not set)
    #[serde(skip_serializing)]
    pub secret: Option<String>,
    /// Token expiration in hours
    #[serde(default = "default_jwt_expiration_hours")]
    pub expiration_hours: u64,
}

/// Registration configuration
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RegistrationConfig {
    /// Allow open registration after bootstrap
    #[serde(default = "default_false")]
    pub allow_open_registration: bool,
    /// Default role for new users
    #[serde(default = "default_user_role")]
    pub default_user_role: String,
    /// Admin roles that have elevated privileges
    #[serde(default = "default_admin_roles")]
    pub admin_roles: Vec<String>,
    /// Bootstrap admin role (role assigned to first user)
    #[serde(default = "default_bootstrap_role")]
    pub bootstrap_admin_role: String,
}

/// TLS forward configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsForwardConfig {
    /// Enable TLS forward functionality
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// List of TLS forward server addresses (NodeAddr format)
    #[serde(default = "default_tlsforward_addresses")]
    pub tlsforward_addresses: Vec<String>,
    /// Maximum concurrent TLS connections
    #[serde(default = "default_tlsforward_max_connections")]
    pub max_connections: usize,
    /// Path to store the secret key for persistent node ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_key_path: Option<String>,
    /// Heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,
    /// Auto-reconnect on disconnect
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,
    /// Maximum reconnection attempts
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: u32,
    /// Reconnection backoff in seconds
    #[serde(default = "default_reconnect_backoff")]
    pub reconnect_backoff: u64,
}

/// Local inference configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalInferenceConfig {
    /// Directory containing model files
    pub models_dir: PathBuf,
    /// List of models to load
    pub models: Vec<LocalModelConfig>,
    /// Maximum concurrent inference requests
    #[serde(default = "default_max_concurrent_inferences")]
    pub max_concurrent_inferences: usize,
}

/// Configuration for a single local model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelConfig {
    /// Model identifier (used in API requests)
    pub id: String,
    /// Display name for the model
    pub name: String,
    /// Path to model file (relative to models_dir)
    pub path: PathBuf,
    /// Model type (e.g., "llama", "mistral", "gpt2")
    pub model_type: String,
    /// Context length in tokens
    pub context_length: u32,
    /// Whether this model supports chat completions
    #[serde(default = "default_true")]
    pub supports_chat: bool,
    /// Whether this model supports text completions
    #[serde(default = "default_true")]
    pub supports_completion: bool,
    /// Default temperature for inference
    #[serde(default = "default_temperature")]
    pub default_temperature: f32,
    /// Default max tokens for inference
    #[serde(default = "default_max_tokens")]
    pub default_max_tokens: u32,
}

fn default_max_concurrent_inferences() -> usize {
    4
}

fn default_temperature() -> f32 {
    0.7
}

fn default_max_tokens() -> u32 {
    2048
}

/// Let's Encrypt configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetsEncryptConfig {
    /// Enable Let's Encrypt certificate management
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Email address for ACME account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Use staging environment for testing
    #[serde(default = "default_false")]
    pub staging: bool,
    /// Domains to request certificates for
    #[serde(default)]
    pub domains: Vec<String>,
    /// Auto-renew certificates before expiry (days)
    #[serde(default = "default_auto_renew_days")]
    pub auto_renew_days: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            cors_origins: Vec::new(),
            metrics_port: None,
        }
    }
}

impl Default for WebAuthnConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            rp_id: default_rp_id(),
            rp_name: default_rp_name(),
            rp_origin: default_rp_origin(),
            allowed_origins: Vec::new(),
            allow_tlsforward_origins: default_true(),
            allow_subdomains: default_true(),
            require_user_verification: default_true(),
            session_timeout_seconds: default_session_timeout(),
        }
    }
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            issuer: default_jwt_issuer(),
            secret: None,
            expiration_hours: default_jwt_expiration_hours(),
        }
    }
}

impl Default for LetsEncryptConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            email: None,
            staging: false,
            domains: Vec::new(),
            auto_renew_days: default_auto_renew_days(),
        }
    }
}

impl Default for TlsForwardConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            tlsforward_addresses: default_tlsforward_addresses(),
            max_connections: default_tlsforward_max_connections(),
            secret_key_path: None,
            heartbeat_interval: default_heartbeat_interval(),
            auto_reconnect: default_true(),
            max_reconnect_attempts: default_max_reconnect_attempts(),
            reconnect_backoff: default_reconnect_backoff(),
        }
    }
}

impl Settings {
    /// Load settings from a specific config file
    pub fn load_from_file(path: &str) -> Result<Self, ConfigError> {
        let mut builder = Config::builder();

        // Start with defaults
        builder = builder.add_source(Config::try_from(&Settings::default())?);

        // Add the specific config file
        builder = builder.add_source(File::with_name(path));

        // Add environment variables with GATE_ prefix (can override file settings)
        builder = builder.add_source(
            Environment::with_prefix("GATE")
                .separator("__")
                .try_parsing(true),
        );

        let config = builder.build()?;
        config.try_deserialize()
    }
}
