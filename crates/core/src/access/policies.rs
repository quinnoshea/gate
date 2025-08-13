use super::identity::{IdentityContext, ObjectIdentity, SubjectIdentity};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Limits that can be applied to allowed actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyLimit {
    TokenLimit(usize),
    RateLimit { per_minute: u32 },
    CreditCost(Decimal),
    Custom(String, serde_json::Value),
}

/// Result of a policy evaluation
#[derive(Debug, Clone)]
pub enum PolicyDecision {
    Allow(Vec<PolicyLimit>),
    Deny(String),
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Policy evaluation failed: {0}")]
    EvaluationFailed(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Generic inference request for policy checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub model: String,
    pub prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub has_images: bool,
}

/// Policy for inference operations
#[async_trait]
pub trait InferencePolicy<C: IdentityContext>: Send + Sync {
    async fn check_inference(
        &self,
        subject: &SubjectIdentity<C>,
        object: &ObjectIdentity,
        request: &InferenceRequest,
    ) -> Result<PolicyDecision, PolicyError>;
}
