use crate::error::{RelayError, Result};
use hellas_gate_core::GateId;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Manages DNS records via Cloudflare API for automatic subdomain provisioning
pub struct DnsManager {
    client: Client,
    api_token: String,
    zone_id: String,
    base_domain: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CloudflareRecord {
    id: Option<String>,
    r#type: String,
    name: String,
    content: String,
    ttl: u32,
    proxied: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CloudflareResponse<T> {
    success: bool,
    errors: Vec<CloudflareError>,
    messages: Vec<String>,
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    code: u32,
    message: String,
}

impl DnsManager {
    /// Create a new DNS manager
    pub async fn new() -> Result<Self> {
        // Load configuration from environment or config file
        let api_token = std::env::var("CLOUDFLARE_API_TOKEN").map_err(|_| {
            RelayError::Config("CLOUDFLARE_API_TOKEN environment variable required".to_string())
        })?;

        let zone_id = std::env::var("CLOUDFLARE_ZONE_ID").map_err(|_| {
            RelayError::Config("CLOUDFLARE_ZONE_ID environment variable required".to_string())
        })?;

        let base_domain =
            std::env::var("RELAY_BASE_DOMAIN").unwrap_or_else(|_| "private.hellas.ai".to_string());

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| RelayError::Config(format!("Failed to create HTTP client: {}", e)))?;

        let manager = Self {
            client,
            api_token,
            zone_id,
            base_domain,
        };

        // Verify API token works
        manager.verify_api_access().await?;

        info!(
            "DNS manager initialized for zone {} ({})",
            manager.zone_id, manager.base_domain
        );

        Ok(manager)
    }

    /// Provision a subdomain for a Gate node using relay's public addresses
    pub async fn provision_subdomain(
        &self,
        node_id: GateId,
        relay_addresses: &[std::net::IpAddr],
    ) -> Result<String> {
        let node_id_hex = hex::encode(node_id.as_bytes());
        let subdomain = format!("{}.{}", node_id_hex, self.base_domain);

        info!(
            "Provisioning subdomain: {} with {} addresses",
            subdomain,
            relay_addresses.len()
        );

        // Create A/AAAA records for all relay addresses
        for ip in relay_addresses {
            let record_type = match ip {
                std::net::IpAddr::V4(_) => "A",
                std::net::IpAddr::V6(_) => "AAAA",
            };

            let record = CloudflareRecord {
                id: None,
                r#type: record_type.to_string(),
                name: subdomain.clone(),
                content: ip.to_string(),
                ttl: 300,             // 5 minute TTL for faster updates
                proxied: Some(false), // Direct connection, not proxied through Cloudflare
            };

            self.create_dns_record(record).await?;
            info!("Created {} record for {}: {}", record_type, subdomain, ip);
        }

        info!("Successfully provisioned subdomain: {}", subdomain);
        Ok(subdomain)
    }

    /// Create a DNS challenge record for Let's Encrypt ACME
    pub async fn create_dns_challenge(&self, domain: &str, token: &str) -> Result<String> {
        let challenge_name = format!("_acme-challenge.{}", domain);

        debug!(
            "Creating DNS challenge record: {} = {}",
            challenge_name, token
        );

        let record = CloudflareRecord {
            id: None,
            r#type: "TXT".to_string(),
            name: challenge_name,
            content: token.to_string(),
            ttl: 120, // 2 minute TTL for ACME challenges
            proxied: Some(false),
        };

        let record_id = self.create_dns_record(record).await?;

        // Wait for DNS propagation
        self.wait_for_dns_propagation(domain, token).await?;

        Ok(record_id)
    }

    /// Remove DNS challenge record after ACME validation
    pub async fn cleanup_dns_challenge(&self, record_id: &str) -> Result<()> {
        debug!("Cleaning up DNS challenge record: {}", record_id);

        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
            self.zone_id, record_id
        );

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.api_token)
            .send()
            .await
            .map_err(|e| RelayError::Dns(format!("Failed to delete DNS record: {}", e)))?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(RelayError::Dns(format!(
                "DNS record deletion failed: {}",
                text
            )));
        }

        debug!("Successfully cleaned up DNS challenge record");
        Ok(())
    }

    /// Remove all DNS records for a subdomain
    pub async fn cleanup_subdomain(&self, domain: &str) -> Result<()> {
        info!("Cleaning up subdomain: {}", domain);

        // List all records for this domain
        let records = self.list_dns_records(Some(domain)).await?;

        // Delete each record
        for record in records {
            if let Some(record_id) = record.id {
                if let Err(e) = self.delete_dns_record(&record_id).await {
                    warn!("Failed to delete DNS record {}: {}", record_id, e);
                }
            }
        }

        info!("Successfully cleaned up subdomain: {}", domain);
        Ok(())
    }

    /// Verify API token has necessary permissions
    async fn verify_api_access(&self) -> Result<()> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            self.zone_id
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.api_token)
            .query(&[("per_page", "1")])
            .send()
            .await
            .map_err(|e| RelayError::Dns(format!("API verification failed: {}", e)))?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(RelayError::Dns(format!(
                "API token verification failed: {}",
                text
            )));
        }

        debug!("Cloudflare API access verified");
        Ok(())
    }

    /// Create a DNS record via Cloudflare API
    async fn create_dns_record(&self, record: CloudflareRecord) -> Result<String> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            self.zone_id
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_token)
            .json(&record)
            .send()
            .await
            .map_err(|e| RelayError::Dns(format!("Failed to create DNS record: {}", e)))?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(RelayError::Dns(format!(
                "DNS record creation failed: {}",
                text
            )));
        }

        let cf_response: CloudflareResponse<CloudflareRecord> = response
            .json()
            .await
            .map_err(|e| RelayError::Dns(format!("Failed to parse DNS response: {}", e)))?;

        if !cf_response.success {
            let errors: Vec<String> = cf_response
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect();
            return Err(RelayError::Dns(format!(
                "DNS API errors: {}",
                errors.join(", ")
            )));
        }

        let created_record = cf_response
            .result
            .ok_or_else(|| RelayError::Dns("No record returned from API".to_string()))?;

        let record_id = created_record
            .id
            .ok_or_else(|| RelayError::Dns("No record ID returned from API".to_string()))?;

        debug!("Created DNS record {} for {}", record_id, record.name);
        Ok(record_id)
    }

    /// List DNS records, optionally filtered by name
    async fn list_dns_records(&self, name: Option<&str>) -> Result<Vec<CloudflareRecord>> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            self.zone_id
        );

        let mut query = vec![("per_page", "100")];
        if let Some(name) = name {
            query.push(("name", name));
        }

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.api_token)
            .query(&query)
            .send()
            .await
            .map_err(|e| RelayError::Dns(format!("Failed to list DNS records: {}", e)))?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(RelayError::Dns(format!(
                "DNS record listing failed: {}",
                text
            )));
        }

        let cf_response: CloudflareResponse<Vec<CloudflareRecord>> = response
            .json()
            .await
            .map_err(|e| RelayError::Dns(format!("Failed to parse DNS response: {}", e)))?;

        if !cf_response.success {
            let errors: Vec<String> = cf_response
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect();
            return Err(RelayError::Dns(format!(
                "DNS API errors: {}",
                errors.join(", ")
            )));
        }

        Ok(cf_response.result.unwrap_or_default())
    }

    /// Delete a DNS record by ID
    async fn delete_dns_record(&self, record_id: &str) -> Result<()> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
            self.zone_id, record_id
        );

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.api_token)
            .send()
            .await
            .map_err(|e| RelayError::Dns(format!("Failed to delete DNS record: {}", e)))?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(RelayError::Dns(format!(
                "DNS record deletion failed: {}",
                text
            )));
        }

        debug!("Deleted DNS record: {}", record_id);
        Ok(())
    }

    /// Wait for DNS propagation using multiple resolvers
    async fn wait_for_dns_propagation(&self, domain: &str, expected_value: &str) -> Result<()> {
        use trust_dns_resolver::{config::*, AsyncResolver};

        // Use Cloudflare's DNS over TLS for faster resolution
        let resolver = AsyncResolver::tokio(ResolverConfig::cloudflare(), ResolverOpts::default());

        let challenge_name = format!("_acme-challenge.{}", domain);
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 30; // 5 minutes with 10 second intervals

        while attempts < MAX_ATTEMPTS {
            debug!(
                "Checking DNS propagation for {} (attempt {})",
                challenge_name,
                attempts + 1
            );

            match resolver.txt_lookup(&challenge_name).await {
                Ok(response) => {
                    for record in response.iter() {
                        if record.to_string().trim_matches('"') == expected_value {
                            info!("DNS propagation confirmed for {}", challenge_name);
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    debug!("DNS lookup failed: {}", e);
                }
            }

            attempts += 1;
            if attempts < MAX_ATTEMPTS {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }

        Err(RelayError::Timeout {
            operation: format!("DNS propagation for {}", challenge_name),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
