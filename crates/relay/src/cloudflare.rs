use crate::error::{RelayError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Manages DNS records via Cloudflare API for ACME challenges
#[derive(Debug)]
pub struct CloudflareDnsManager {
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
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    code: u32,
    message: String,
}

impl CloudflareDnsManager {
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

    /// Create a DNS challenge record for Let's Encrypt ACME
    pub async fn create_dns_challenge(&self, domain: &str, token: &str) -> Result<String> {
        let challenge_name = domain.to_string();

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

        // Return immediately after record creation
        Ok(record_id)
    }

    /// Remove DNS challenge record after ACME validation by record ID
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

    /// Remove DNS challenge record by domain name
    pub async fn cleanup_dns_challenge_by_domain(&self, domain: &str) -> Result<()> {
        let challenge_name = domain.to_string();
        info!(
            "Cleaning up DNS challenge record for domain: {}",
            challenge_name
        );

        // List records to find the challenge record
        let records = self.list_dns_records(Some(&challenge_name)).await?;

        // Delete any TXT records for the challenge
        for record in records {
            if record.r#type == "TXT" && record.name == challenge_name {
                if let Some(record_id) = record.id {
                    if let Err(e) = self.cleanup_dns_challenge(&record_id).await {
                        warn!("Failed to delete challenge record {}: {}", record_id, e);
                    } else {
                        info!("Successfully cleaned up challenge record for {}", domain);
                    }
                }
            }
        }

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

    /// Check DNS propagation status (non-blocking, single check)
    pub async fn check_dns_propagation(&self, domain: &str, expected_value: &str) -> Result<bool> {
        use trust_dns_resolver::{config::*, AsyncResolver};

        // Use Cloudflare's DNS over TLS for faster resolution
        let resolver = AsyncResolver::tokio(ResolverConfig::cloudflare(), ResolverOpts::default());

        let challenge_name = domain.to_string();

        debug!("Checking DNS propagation for {}", challenge_name);

        match resolver.txt_lookup(&challenge_name).await {
            Ok(response) => {
                for record in response.iter() {
                    if record.to_string().trim_matches('"') == expected_value {
                        debug!("DNS propagation confirmed for {}", challenge_name);
                        return Ok(true);
                    }
                }
                debug!(
                    "DNS record found but value doesn't match for {}",
                    challenge_name
                );
                Ok(false)
            }
            Err(e) => {
                debug!("DNS lookup failed for {}: {}", challenge_name, e);
                Ok(false)
            }
        }
    }
}
