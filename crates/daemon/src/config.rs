//! Server configuration

use config::{Config, ConfigError, Environment, File};
use gate_core::{ValidateConfig, validators};
use gate_http::forwarding::UpstreamProvider;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Server settings
    pub server: ServerConfig,
    /// Database settings
    pub database: DatabaseConfig,
    /// Plugin settings
    pub plugins: PluginConfig,
    /// Authentication settings
    #[serde(default)]
    pub auth: AuthConfig,
    /// Upstream provider settings (supports multiple)
    #[serde(default)]
    pub upstreams: Vec<UpstreamConfig>,
    /// Relay configuration
    #[serde(default)]
    pub tlsforward: TlsForwardConfig,
    /// Let's Encrypt configuration
    #[serde(default)]
    pub letsencrypt: LetsEncryptConfig,
    /// Optional state directory override (uses platform defaults if not set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_dir: Option<String>,
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

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,
    /// Maximum number of connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Enable plugin system
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Plugin directories
    #[serde(default)]
    pub directories: Vec<String>,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    #[serde(default)]
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
    #[serde(default)]
    pub allow_subdomains: bool,
    /// Require user verification
    #[serde(default)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationConfig {
    /// Allow open registration after bootstrap
    #[serde(default)]
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
    #[serde(default)]
    pub enabled: bool,
    /// Addresses of TLS forward servers (NodeAddr format)
    /// List of TLS forward server addresses (NodeAddr format)
    #[serde(default)]
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

/// Let's Encrypt configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetsEncryptConfig {
    /// Enable Let's Encrypt certificate management
    #[serde(default)]
    pub enabled: bool,
    /// Email address for ACME account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Use staging environment for testing
    #[serde(default)]
    pub staging: bool,
    /// Domains to request certificates for
    #[serde(default)]
    pub domains: Vec<String>,
    /// Auto-renew certificates before expiry (days)
    #[serde(default = "default_auto_renew_days")]
    pub auto_renew_days: u32,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_max_connections() -> u32 {
    10
}

fn default_true() -> bool {
    true
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

impl Default for WebAuthnConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rp_id: default_rp_id(),
            rp_name: default_rp_name(),
            rp_origin: default_rp_origin(),
            allowed_origins: vec![],
            allow_tlsforward_origins: true,
            allow_subdomains: false,
            require_user_verification: false,
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

impl Default for RegistrationConfig {
    fn default() -> Self {
        Self {
            allow_open_registration: false,
            default_user_role: default_user_role(),
            admin_roles: default_admin_roles(),
            bootstrap_admin_role: default_bootstrap_role(),
        }
    }
}

impl Default for TlsForwardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tlsforward_addresses: vec![],
            max_connections: default_tlsforward_max_connections(),
            secret_key_path: None,
            heartbeat_interval: default_heartbeat_interval(),
            auto_reconnect: true,
            max_reconnect_attempts: default_max_reconnect_attempts(),
            reconnect_backoff: default_reconnect_backoff(),
        }
    }
}

impl Default for LetsEncryptConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            email: None,
            staging: false,
            domains: vec![],
            auto_renew_days: default_auto_renew_days(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
                cors_origins: vec![],
                metrics_port: None,
            },
            database: DatabaseConfig {
                url: "sqlite://gate.db".to_string(),
                max_connections: default_max_connections(),
            },
            plugins: PluginConfig {
                enabled: true,
                directories: vec![],
            },
            auth: AuthConfig::default(),
            upstreams: vec![],
            tlsforward: TlsForwardConfig::default(),
            letsencrypt: LetsEncryptConfig::default(),
            state_dir: None,
        }
    }
}

impl Settings {
    /// Load settings from environment and config files
    pub fn new() -> Result<Self, ConfigError> {
        let mut builder = Config::builder();

        // Try to find config files in common locations
        let config_paths = ["gate.toml", "config/gate.toml", "/etc/gate/gate.toml"];

        for path in &config_paths {
            if Path::new(path).exists() {
                tracing::info!("Loading configuration from: {}", path);
                builder = builder.add_source(File::with_name(path).required(false));
            }
        }

        // Add environment variables with GATE_ prefix
        builder = builder.add_source(
            Environment::with_prefix("GATE")
                .separator("__")
                .try_parsing(true),
        );

        let config = builder.build()?;
        config.try_deserialize()
    }

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

    /// Merge runtime configuration on top of existing settings
    pub fn merge_runtime_config(mut self, runtime_path: &Path) -> Result<Self, ConfigError> {
        if runtime_path.exists() {
            let runtime_config = Config::builder()
                .add_source(File::from(runtime_path))
                .build()?;

            // Convert self to Value, merge, and convert back
            let base_value = serde_json::to_value(&self).map_err(|e| {
                ConfigError::Message(format!("Failed to serialize base config: {e}"))
            })?;
            let runtime_value: serde_json::Value = runtime_config.try_deserialize()?;

            // Merge runtime config on top of base config
            let merged = merge_json_values(base_value, runtime_value);

            self = serde_json::from_value(merged).map_err(|e| {
                ConfigError::Message(format!("Failed to deserialize merged config: {e}"))
            })?;
        }

        Ok(self)
    }
}

/// Merge two JSON values, with the second value taking precedence
fn merge_json_values(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (key, overlay_value) in overlay_map {
                match base_map.get(&key) {
                    Some(base_value) if base_value.is_object() && overlay_value.is_object() => {
                        // Recursively merge objects
                        base_map.insert(key, merge_json_values(base_value.clone(), overlay_value));
                    }
                    _ => {
                        // Replace the value
                        base_map.insert(key, overlay_value);
                    }
                }
            }
            Value::Object(base_map)
        }
        (_, overlay) => overlay, // Overlay completely replaces base for non-objects
    }
}

impl ValidateConfig for Settings {
    fn validate(&self) -> Result<(), ConfigError> {
        // Server validation
        validators::validate_port(self.server.port, "server.port")?;
        validators::validate_not_empty(&self.server.host, "server.host")?;

        // Database validation
        validators::validate_not_empty(&self.database.url, "database.url")?;
        validators::validate_range(
            self.database.max_connections,
            1,
            1000,
            "database.max_connections",
        )?;

        // Upstream validation
        for (i, upstream) in self.upstreams.iter().enumerate() {
            let prefix = format!("upstreams[{i}]");

            validators::validate_not_empty(&upstream.name, &format!("{prefix}.name"))?;
            validators::validate_url(&upstream.base_url, &format!("{prefix}.base_url"))?;
            validators::validate_range(
                upstream.timeout_seconds,
                1,
                300,
                &format!("{prefix}.timeout_seconds"),
            )?;

            // Check for duplicate upstream names
            let duplicate_count = self
                .upstreams
                .iter()
                .filter(|u| u.name == upstream.name)
                .count();
            if duplicate_count > 1 {
                return Err(ConfigError::Message(format!(
                    "Duplicate upstream name: {}",
                    upstream.name
                )));
            }
        }

        // Auth validation
        if self.auth.webauthn.enabled {
            validators::validate_not_empty(&self.auth.webauthn.rp_id, "auth.webauthn.rp_id")?;
            validators::validate_not_empty(&self.auth.webauthn.rp_name, "auth.webauthn.rp_name")?;
            validators::validate_url(&self.auth.webauthn.rp_origin, "auth.webauthn.rp_origin")?;

            for (i, origin) in self.auth.webauthn.allowed_origins.iter().enumerate() {
                validators::validate_url(origin, &format!("auth.webauthn.allowed_origins[{i}]"))?;
            }
        }

        // JWT validation
        validators::validate_not_empty(&self.auth.jwt.issuer, "auth.jwt.issuer")?;
        validators::validate_range(
            self.auth.jwt.expiration_hours,
            1,
            24 * 365, // Max 1 year
            "auth.jwt.expiration_hours",
        )?;

        // Relay validation
        if self.tlsforward.enabled {
            validators::validate_range(
                self.tlsforward.heartbeat_interval,
                5,
                300,
                "relay.heartbeat_interval",
            )?;
            validators::validate_range(
                self.tlsforward.max_reconnect_attempts,
                0,
                100,
                "relay.max_reconnect_attempts",
            )?;
            validators::validate_range(
                self.tlsforward.reconnect_backoff,
                1,
                60,
                "relay.reconnect_backoff",
            )?;
        }

        // Let's Encrypt validation
        if self.letsencrypt.enabled {
            if let Some(email) = &self.letsencrypt.email {
                validators::validate_email(email, "letsencrypt.email")?;
            } else {
                return Err(ConfigError::Message(
                    "letsencrypt.email is required when Let's Encrypt is enabled".to_string(),
                ));
            }

            // Allow empty domains when relay is enabled - relay will add its domain automatically
            if self.letsencrypt.domains.is_empty() && !self.tlsforward.enabled {
                return Err(ConfigError::Message(
                    "At least one domain is required when Let's Encrypt is enabled (or enable relay for automatic domain assignment)".to_string(),
                ));
            }

            validators::validate_range(
                self.letsencrypt.auto_renew_days,
                7,
                90,
                "letsencrypt.auto_renew_days",
            )?;
        }

        Ok(())
    }
}
