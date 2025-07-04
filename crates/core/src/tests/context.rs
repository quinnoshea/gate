//! Mock implementation of RequestContext for testing

use crate::RequestContext;
use async_trait::async_trait;
use std::collections::HashMap;

/// Mock implementation of RequestContext for testing
#[derive(Clone)]
pub struct MockRequestContext {
    pub env_vars: HashMap<String, String>,
    pub secrets: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub url: String,
    pub method: String,
    pub body: Vec<u8>,
    pub client_ip: Option<String>,
}

impl Default for MockRequestContext {
    fn default() -> Self {
        Self {
            env_vars: HashMap::new(),
            secrets: HashMap::new(),
            headers: HashMap::new(),
            url: "http://localhost:3000".to_string(),
            method: "GET".to_string(),
            body: Vec::new(),
            client_ip: Some("127.0.0.1".to_string()),
        }
    }
}

#[async_trait]
impl RequestContext for MockRequestContext {
    async fn get_env_var(&self, key: &str) -> Option<String> {
        self.env_vars.get(key).cloned()
    }

    async fn get_secret(&self, key: &str) -> Option<String> {
        self.secrets.get(key).cloned()
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

    async fn get_body(
        &self,
    ) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.body.clone())
    }

    fn get_client_ip(&self) -> Option<String> {
        self.client_ip.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_request_context() {
        let mut ctx = MockRequestContext::default();

        // Test env vars
        ctx.env_vars
            .insert("TEST_VAR".to_string(), "test_value".to_string());
        assert_eq!(
            ctx.get_env_var("TEST_VAR").await,
            Some("test_value".to_string())
        );
        assert_eq!(ctx.get_env_var("MISSING").await, None);

        // Test secrets
        ctx.secrets
            .insert("SECRET_KEY".to_string(), "secret_value".to_string());
        assert_eq!(
            ctx.get_secret("SECRET_KEY").await,
            Some("secret_value".to_string())
        );

        // Test headers
        ctx.headers
            .insert("Content-Type".to_string(), "application/json".to_string());
        assert_eq!(
            ctx.get_headers().get("Content-Type"),
            Some(&"application/json".to_string())
        );

        // Test URL and method
        assert_eq!(ctx.get_url(), "http://localhost:3000");
        assert_eq!(ctx.get_method(), "GET");

        // Test body
        ctx.body = b"test body".to_vec();
        assert_eq!(ctx.get_body().await.unwrap(), b"test body");

        // Test client IP
        assert_eq!(ctx.get_client_ip(), Some("127.0.0.1".to_string()));
    }
}
