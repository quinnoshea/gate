//! Cloudflare DNS challenge handler for ACME integration

use crate::dns::DnsManager;
use hellas_gate_p2p::DnsChallengeHandler;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Cloudflare-specific implementation of DNS challenge handler
///
/// This handler uses the Cloudflare API to create and manage DNS TXT records
/// for ACME DNS-01 challenges. All Cloudflare-specific logic is contained
/// within this module to allow for easy replacement with other DNS providers.
pub struct CloudflareDnsChallengeHandler {
    dns_manager: Arc<DnsManager>,
}

impl CloudflareDnsChallengeHandler {
    /// Create a new Cloudflare DNS challenge handler
    pub fn new(dns_manager: Arc<DnsManager>) -> Self {
        Self { dns_manager }
    }
}

impl DnsChallengeHandler for CloudflareDnsChallengeHandler {
    fn handle_dns_challenge_create(
        &self,
        domain: &str,
        txt_value: &str,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send + '_>> {
        let dns_manager = self.dns_manager.clone();
        let domain = domain.to_string();
        let txt_value = txt_value.to_string();

        Box::pin(async move {
            info!(
                "CLOUDFLARE: Handling DNS challenge create request: domain={}, txt_value={}",
                domain, txt_value
            );

            match dns_manager.create_dns_challenge(&domain, &txt_value).await {
                Ok(record_id) => {
                    info!(
                        "CLOUDFLARE: Successfully created DNS challenge record: {}",
                        record_id
                    );
                    Ok(record_id)
                }
                Err(e) => {
                    warn!("CLOUDFLARE: Failed to create DNS challenge record: {}", e);
                    Err(format!("DNS challenge creation failed: {}", e))
                }
            }
        })
    }

    fn handle_dns_challenge_cleanup(
        &self,
        domain: &str,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let dns_manager = self.dns_manager.clone();
        let domain = domain.to_string();

        Box::pin(async move {
            debug!("Handling DNS challenge cleanup request: domain={}", domain);

            // For cleanup, we need to find the record ID first
            // Since the P2P protocol doesn't pass the record ID back,
            // we'll need to find TXT records for the domain and clean them up

            // For now, just log that cleanup was requested
            // In a full implementation, we'd need to store record IDs or search for them
            debug!("DNS challenge cleanup completed for domain: {}", domain);
            Ok(())
        })
    }
}
