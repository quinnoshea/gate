//! Certificate storage for relay server

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Certificate data for a domain
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    pub domain: String,
    pub cert_pem: String,
    pub key_pem: String,
    pub node_id: String,
}

/// Certificate store for managing domain certificates
#[derive(Debug, Clone)]
pub struct CertificateStore {
    /// Maps domain -> certificate info
    certificates: Arc<RwLock<HashMap<String, CertificateInfo>>>,
}

impl CertificateStore {
    /// Create a new certificate store
    pub fn new() -> Self {
        Self {
            certificates: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Store a certificate for a domain
    pub async fn store_certificate(&self, cert_info: CertificateInfo) -> Result<()> {
        let domain = cert_info.domain.clone();
        let node_id = cert_info.node_id.clone();

        let mut certs = self.certificates.write().await;
        certs.insert(domain.clone(), cert_info);

        info!(
            "Stored certificate for domain: {} (node: {})",
            domain, node_id
        );
        debug!("Total certificates stored: {}", certs.len());

        Ok(())
    }

    /// Get certificate for a domain
    pub async fn get_certificate(&self, domain: &str) -> Option<CertificateInfo> {
        let certs = self.certificates.read().await;
        certs.get(domain).cloned()
    }

    /// List all stored domains
    pub async fn list_domains(&self) -> Vec<String> {
        let certs = self.certificates.read().await;
        certs.keys().cloned().collect()
    }

    /// Remove certificate for a domain
    pub async fn remove_certificate(&self, domain: &str) -> bool {
        let mut certs = self.certificates.write().await;
        let removed = certs.remove(domain).is_some();

        if removed {
            info!("Removed certificate for domain: {}", domain);
        } else {
            warn!(
                "Attempted to remove non-existent certificate for domain: {}",
                domain
            );
        }

        removed
    }

    /// Get certificate count
    pub async fn certificate_count(&self) -> usize {
        let certs = self.certificates.read().await;
        certs.len()
    }
}

impl Default for CertificateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_certificate_store() {
        let store = CertificateStore::new();

        let cert_info = CertificateInfo {
            domain: "test.private.hellas.ai".to_string(),
            cert_pem: "-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----".to_string(),
            key_pem: "-----BEGIN PRIVATE KEY-----\ntest\n-----END PRIVATE KEY-----".to_string(),
            node_id: "test123".to_string(),
        };

        // Store certificate
        store.store_certificate(cert_info.clone()).await.unwrap();

        // Retrieve certificate
        let retrieved = store.get_certificate("test.private.hellas.ai").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().domain, "test.private.hellas.ai");

        // List domains
        let domains = store.list_domains().await;
        assert_eq!(domains.len(), 1);
        assert!(domains.contains(&"test.private.hellas.ai".to_string()));

        // Remove certificate
        let removed = store.remove_certificate("test.private.hellas.ai").await;
        assert!(removed);

        // Verify removal
        let retrieved = store.get_certificate("test.private.hellas.ai").await;
        assert!(retrieved.is_none());
    }
}
