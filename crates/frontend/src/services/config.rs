//! Configuration API service

use crate::client::get_client;
use serde_json::Value;

/// Configuration API service
#[derive(Clone)]
pub struct ConfigApiService;

impl ConfigApiService {
    /// Create a new config API service
    pub fn new() -> Self {
        Self
    }

    /// Get the full configuration
    pub async fn get_config(&self) -> Result<Value, String> {
        let client = get_client().map_err(|e| format!("Failed to get client: {e}"))?;

        client
            .get_config()
            .await
            .map(|response| response.config)
            .map_err(|e| e.to_string())
    }

    /// Update the full configuration
    pub async fn update_config(&self, config: Value) -> Result<Value, String> {
        let client = get_client().map_err(|e| format!("Failed to get client: {e}"))?;

        client
            .update_config(config)
            .await
            .map(|response| response.config)
            .map_err(|e| e.to_string())
    }
}
