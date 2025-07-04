//! WebAuthn types and traits

use crate::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// WebAuthn credential stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    pub credential_id: String,
    pub user_id: String,
    pub public_key: Vec<u8>,
    pub aaguid: Option<Vec<u8>>,
    pub counter: u32,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub device_name: Option<String>,
}

/// Trait for WebAuthn credential storage
#[async_trait]
pub trait WebAuthnBackend: Send + Sync {
    /// Store a new WebAuthn credential
    async fn store_webauthn_credential(&self, credential: &StoredCredential) -> Result<()>;

    /// Get a credential by ID
    async fn get_webauthn_credential(
        &self,
        credential_id: &str,
    ) -> Result<Option<StoredCredential>>;

    /// List all credentials for a user
    async fn list_user_credentials(&self, user_id: &str) -> Result<Vec<StoredCredential>>;

    /// List all credentials (for authentication)
    async fn list_all_credentials(&self) -> Result<Vec<StoredCredential>>;

    /// Update credential counter and last used timestamp
    async fn update_credential_counter(&self, credential_id: &str, counter: u32) -> Result<()>;

    /// Delete a credential
    async fn delete_credential(&self, credential_id: &str) -> Result<()>;
}
