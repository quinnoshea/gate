//! Upstream inference provider client

use crate::config::UpstreamConfig;
use crate::{DaemonError, ErrorContext, Result};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::time::Duration;
use tracing::{debug, info};

/// Client for communicating with upstream inference providers
#[derive(Debug, Clone)]
pub struct UpstreamClient {
    client: Client,
    config: UpstreamConfig,
}

/// Wrapper around inference request with extracted model name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    /// The raw request payload
    pub payload: JsonValue,

    /// Extracted model name
    pub model: String,
}

/// Wrapper around upstream response with optional extracted metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamResponse {
    /// The raw response from upstream provider
    pub response: JsonValue,

    /// Extracted usage information if available
    pub usage: Option<JsonValue>,

    /// Extracted model name if available
    pub model: Option<String>,
}

impl UpstreamClient {
    /// Create a new upstream client
    ///
    /// # Errors
    ///
    /// Returns an error if client initialization fails
    pub fn new(config: &UpstreamConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .with_context_str("Failed to create HTTP client")
            .map_err(|e| DaemonError::Upstream(e))?;

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// List available models from the upstream provider
    ///
    /// # Errors
    ///
    /// Returns an error if the upstream request fails
    pub async fn list_models(&self) -> Result<JsonValue> {
        info!(
            "Fetching available models from upstream: {}",
            self.config.default_url
        );

        // Build the HTTP request
        let mut http_request = self
            .client
            .get(format!("{}/models", self.config.default_url))
            .header("Content-Type", "application/json");

        // Add authorization if API key is configured
        if let Some(api_key) = &self.config.api_key {
            http_request = http_request.header("Authorization", format!("Bearer {api_key}"));
        }

        // Send the request
        let response = http_request
            .send()
            .await
            .map_err(|e| DaemonError::Upstream(format!("Models request failed: {e}")))?;

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(DaemonError::Upstream(format!(
                "Models endpoint returned {status}: {body}"
            )));
        }

        // Parse the response as raw JSON
        let response_json: JsonValue = response
            .json()
            .await
            .with_context_str("Failed to parse models response")
            .map_err(|e| DaemonError::Upstream(e))?;

        debug!("Available models: {}", response_json);

        Ok(response_json)
    }

    /// Send a chat completion request to the upstream provider
    ///
    /// # Errors
    ///
    /// Returns an error if the upstream request fails
    pub async fn chat_completion(&self, request: InferenceRequest) -> Result<UpstreamResponse> {
        info!(
            "Forwarding inference request to upstream: {}",
            self.config.default_url
        );

        debug!("Upstream request payload: {}", request.payload);

        // Build the HTTP request
        let mut http_request = self
            .client
            .post(format!("{}/chat/completions", self.config.default_url))
            .header("Content-Type", "application/json")
            .json(&request.payload);

        // Add authorization if API key is configured
        if let Some(api_key) = &self.config.api_key {
            http_request = http_request.header("Authorization", format!("Bearer {api_key}"));
        }

        // Send the request
        let response = http_request
            .send()
            .await
            .map_err(|e| DaemonError::Upstream(format!("Request failed: {e}")))?;

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(DaemonError::Upstream(format!(
                "Upstream returned {status}: {body}"
            )));
        }

        // Parse the response as raw JSON
        let response_json: JsonValue = response
            .json()
            .await
            .map_err(|e| DaemonError::Upstream(format!("Failed to parse response: {e}")))?;

        debug!("Upstream response: {}", response_json);

        // Extract optional metadata
        let usage = response_json.get("usage").cloned();
        let model = response_json
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(UpstreamResponse {
            response: response_json,
            usage,
            model,
        })
    }
}

impl InferenceRequest {
    /// Create a new inference request from JSON payload
    ///
    /// # Errors
    ///
    /// Returns an error if the model field cannot be extracted
    pub fn new(payload: JsonValue) -> Result<Self> {
        let model = payload
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DaemonError::Other(anyhow::anyhow!("Missing 'model' field in request")))?
            .to_string();

        Ok(Self { payload, model })
    }

    /// Get the model name
    #[must_use]
    pub const fn model(&self) -> &String {
        &self.model
    }

    /// Get the raw payload
    #[must_use]
    pub const fn payload(&self) -> &JsonValue {
        &self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_request_creation() {
        let payload = serde_json::json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "hello"}]
        });

        let request = InferenceRequest::new(payload.clone()).unwrap();
        assert_eq!(request.model(), "test-model");
        assert_eq!(request.payload(), &payload);
    }

    #[test]
    fn test_inference_request_missing_model() {
        let payload = serde_json::json!({
            "messages": [{"role": "user", "content": "hello"}]
        });

        let result = InferenceRequest::new(payload);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_lm_studio_models_endpoint() {
        // This test assumes LM Studio is running on localhost:1234
        let config = UpstreamConfig {
            default_url: "http://localhost:1234/v1".to_string(),
            timeout_secs: 30,
            api_key: None,
            test_model: "test-model".to_string(),
        };

        let client = UpstreamClient::new(&config).unwrap();

        match client.list_models().await {
            Ok(models) => {
                println!("LM Studio models endpoint success!");
                println!(
                    "Models response: {}",
                    serde_json::to_string_pretty(&models).unwrap()
                );

                // Verify basic structure
                assert!(models.get("object").is_some());
                assert!(models.get("data").is_some());
            }
            Err(e) => {
                println!("LM Studio connection failed: {e}");
                println!("Make sure LM Studio is running on localhost:1234");
                // Don't fail the test if LM Studio isn't running
            }
        }
    }

    #[tokio::test]
    async fn test_lm_studio_chat_completion() {
        // This test assumes LM Studio is running with a loaded model
        let config = UpstreamConfig {
            default_url: "http://localhost:1234/v1".to_string(),
            timeout_secs: 30,
            api_key: None,
            test_model: "test-model".to_string(),
        };

        let client = UpstreamClient::new(&config).unwrap();

        let request_payload = serde_json::json!({
            "model": "test-model", // LM Studio usually ignores the model name
            "messages": [
                {
                    "role": "user",
                    "content": "Say 'Hello from LM Studio!' and nothing else."
                }
            ],
            "max_tokens": 50,
            "temperature": 0.1
        });

        let request = InferenceRequest::new(request_payload).unwrap();

        match client.chat_completion(request).await {
            Ok(response) => {
                println!("LM Studio chat completion success!");
                println!(
                    "Response: {}",
                    serde_json::to_string_pretty(&response.response).unwrap()
                );

                // Verify basic OpenAI structure
                assert!(response.response.get("choices").is_some());
                assert!(response.response.get("object").is_some());
            }
            Err(e) => {
                println!("LM Studio chat completion failed: {e}");
                println!("Make sure LM Studio is running with a loaded model on localhost:1234");
                // Don't fail the test if LM Studio isn't running or no model loaded
            }
        }
    }
}
