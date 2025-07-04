use async_trait::async_trait;
use gate_core::RequestContext;
use std::collections::HashMap;
use std::env;

/// Native server implementation of RequestContext
pub struct NativeRequestContext {
    headers: HashMap<String, String>,
    url: String,
    method: String,
    body: Option<Vec<u8>>,
}

impl NativeRequestContext {
    pub fn new(
        headers: HashMap<String, String>,
        url: String,
        method: String,
        body: Option<Vec<u8>>,
    ) -> Self {
        Self {
            headers,
            url,
            method,
            body,
        }
    }

    /// Create from an Axum request
    pub async fn from_axum_request(
        req: axum::extract::Request,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let method = req.method().to_string();
        let url = req.uri().to_string();

        // Extract headers
        let mut headers = HashMap::new();
        for (name, value) in req.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        // Extract body
        let body = axum::body::to_bytes(req.into_body(), usize::MAX)
            .await
            .map(|b| b.to_vec())
            .ok();

        Ok(Self {
            headers,
            url,
            method,
            body,
        })
    }
}

#[async_trait]
impl RequestContext for NativeRequestContext {
    async fn get_env_var(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }

    async fn get_secret(&self, key: &str) -> Option<String> {
        // In production, this might read from a secure vault
        // For now, we'll use environment variables with a SECRET_ prefix
        env::var(format!("SECRET_{key}")).ok()
    }

    fn get_headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    fn get_url(&self) -> &str {
        &self.url
    }

    fn get_method(&self) -> &str {
        &self.method
    }

    async fn get_body(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        self.body.clone().ok_or_else(|| "No body available".into())
    }

    fn get_client_ip(&self) -> Option<String> {
        // Check common headers for client IP
        self.headers
            .get("x-forwarded-for")
            .and_then(|v| v.split(',').next())
            .map(|ip| ip.trim().to_string())
            .or_else(|| self.headers.get("x-real-ip").cloned())
            .or_else(|| self.headers.get("x-client-ip").cloned())
    }
}
