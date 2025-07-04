//! Forwarding configuration for HTTP inference routes

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for forwarding requests to upstream providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardingConfig {
    /// Provider type
    pub provider: UpstreamProvider,
    /// Base URL for the upstream API
    pub base_url: String,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

/// Supported upstream LLM providers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpstreamProvider {
    Anthropic,
    OpenAI,
    Custom,
}

impl fmt::Display for UpstreamProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpstreamProvider::Anthropic => write!(f, "anthropic"),
            UpstreamProvider::OpenAI => write!(f, "openai"),
            UpstreamProvider::Custom => write!(f, "custom"),
        }
    }
}

impl ForwardingConfig {
    /// Get the appropriate authorization header for the provider
    pub fn auth_header(&self) -> Option<(&'static str, String)> {
        self.api_key.as_ref().map(|key| match self.provider {
            UpstreamProvider::Anthropic => ("x-api-key", key.clone()),
            UpstreamProvider::OpenAI => ("Authorization", format!("Bearer {key}")),
            UpstreamProvider::Custom => ("Authorization", format!("Bearer {key}")),
        })
    }

    /// Get provider-specific headers
    pub fn provider_headers(&self) -> Vec<(&'static str, &'static str)> {
        match self.provider {
            UpstreamProvider::Anthropic => vec![("anthropic-version", "2023-06-01")],
            _ => vec![],
        }
    }
}

/// Registry for managing multiple upstream providers
#[derive(Clone)]
pub struct UpstreamRegistry {
    /// Map of upstream name to configuration
    upstreams: Arc<RwLock<HashMap<String, UpstreamInfo>>>,
    /// Map of model to upstream name
    model_mapping: Arc<RwLock<HashMap<String, String>>>,
}

/// Information about an upstream provider
#[derive(Debug, Clone)]
pub struct UpstreamInfo {
    pub config: ForwardingConfig,
    pub models: Vec<String>,
}

impl Default for UpstreamRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl UpstreamRegistry {
    /// Create a new upstream registry
    pub fn new() -> Self {
        Self {
            upstreams: Arc::new(RwLock::new(HashMap::new())),
            model_mapping: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an upstream with its configuration and models
    pub async fn register_upstream(
        &self,
        name: String,
        config: ForwardingConfig,
        models: Vec<String>,
    ) {
        let mut upstreams = self.upstreams.write().await;
        let mut model_mapping = self.model_mapping.write().await;

        // Add upstream info
        upstreams.insert(
            name.clone(),
            UpstreamInfo {
                config,
                models: models.clone(),
            },
        );

        // Update model mapping
        for model in models {
            model_mapping.insert(model, name.clone());
        }
    }

    /// Get the upstream configuration for a specific model
    pub async fn get_upstream_for_model(&self, model: &str) -> Option<ForwardingConfig> {
        let model_mapping = self.model_mapping.read().await;
        if let Some(upstream_name) = model_mapping.get(model) {
            let upstreams = self.upstreams.read().await;
            upstreams.get(upstream_name).map(|info| info.config.clone())
        } else {
            None
        }
    }

    /// Get all registered upstreams
    pub async fn get_all_upstreams(&self) -> Vec<(String, UpstreamInfo)> {
        let upstreams = self.upstreams.read().await;
        upstreams
            .iter()
            .map(|(name, info)| (name.clone(), info.clone()))
            .collect()
    }

    /// Check if any upstreams are registered
    pub async fn has_upstreams(&self) -> bool {
        let upstreams = self.upstreams.read().await;
        !upstreams.is_empty()
    }

    /// Clear all registered upstreams
    pub async fn clear(&self) {
        let mut upstreams = self.upstreams.write().await;
        let mut model_mapping = self.model_mapping.write().await;

        upstreams.clear();
        model_mapping.clear();
    }
}
