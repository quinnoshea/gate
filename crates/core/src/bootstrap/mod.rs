//! Bootstrap token management
//!
//! This module provides functionality for managing bootstrap tokens,
//! including parsing them from log files for automated discovery.

use async_trait::async_trait;

pub mod parser;

pub use parser::BootstrapTokenParser;

/// Trait for managing bootstrap tokens for initial user enrollment
#[async_trait]
pub trait BootstrapTokenValidator: Send + Sync {
    /// Validate a bootstrap token
    async fn validate_token(
        &self,
        token: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Mark the bootstrap token as used
    async fn mark_token_as_used(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Check if bootstrap has been completed
    async fn is_bootstrap_complete(&self) -> bool;

    /// Check if any credentials exist (to determine if bootstrap is needed)
    async fn needs_bootstrap(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}
