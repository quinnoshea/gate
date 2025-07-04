use async_trait::async_trait;
use std::collections::HashMap;

/// Platform-agnostic request context trait
///
/// This trait abstracts away platform-specific details like Cloudflare's worker::Env
/// to allow the same business logic to run on different platforms.
#[async_trait]
pub trait RequestContext: Send + Sync {
    /// Get an environment variable
    async fn get_env_var(&self, key: &str) -> Option<String>;

    /// Get a secret value
    async fn get_secret(&self, key: &str) -> Option<String>;

    /// Get request headers
    fn get_headers(&self) -> &HashMap<String, String>;

    /// Get the request URL
    fn get_url(&self) -> &str;

    /// Get the request method
    fn get_method(&self) -> &str;

    /// Get the request body as bytes
    async fn get_body(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>;

    /// Get client IP address
    fn get_client_ip(&self) -> Option<String>;
}
