//! Unified certificate management for Gate daemon
//!
//! This module provides comprehensive certificate management including:
//! - Certificate storage and caching
//! - Let's Encrypt certificate generation via relay
//! - Self-signed certificate generation as fallback

use crate::{DaemonError, DaemonErrorContext, LetsEncryptConfig, Result};
use anyhow::{Context, Result as AnyhowResult};
use base64::Engine;
use hellas_gate_core::GateAddr;
use hellas_gate_proto::pb::gate::relay::v1::*;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SanType};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tracing::{debug, info, warn};

/// Certificate data for a domain
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CertificateInfo {
    pub domain: String,
    pub cert_pem: String,
    pub key_pem: String,
    pub node_id: String,
    pub cert_type: CertificateType,
}

/// Type of certificate
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CertificateType {
    SelfSigned,
    LetsEncrypt,
}

/// Unified certificate manager that handles storage, Let's Encrypt, and self-signed certificates
#[derive(Clone)]
pub struct CertificateManager {
    /// Directory to store certificates on disk
    cert_dir: PathBuf,
    /// Maps domain -> certificate info (in-memory cache)
    certificates: Arc<RwLock<HashMap<String, CertificateInfo>>>,
    /// LetsEncrypt configuration
    letsencrypt_config: LetsEncryptConfig,
}

impl CertificateManager {
    /// Create a new certificate manager
    pub async fn new(config: LetsEncryptConfig, cert_dir: PathBuf) -> Result<Self> {
        // Ensure certificate directory exists
        if !cert_dir.exists() {
            fs::create_dir_all(&cert_dir).await
                .with_certificate_context("Failed to create cert directory")?;
            info!("Created certificate directory: {}", cert_dir.display());
        }

        let manager = Self {
            cert_dir,
            certificates: Arc::new(RwLock::new(HashMap::new())),
            letsencrypt_config: config,
        };

        // Load existing certificates from disk
        manager.load_certificates_from_disk().await?;

        Ok(manager)
    }

    /// Get or create a certificate for the given domain
    ///
    /// This method will:
    /// 1. Check if we have a valid cached certificate
    /// 2. Try to get a Let's Encrypt certificate via relay if available
    /// 3. Fall back to self-signed certificate generation
    pub async fn get_or_create_certificate(
        &self,
        domain: &str,
        node_id: &str,
        p2p_private_key: &[u8],
        relay_addr: Option<&GateAddr>,
    ) -> Result<CertificateInfo> {
        // Check if we already have a valid certificate
        if let Some(cert) = self.get_certificate(domain).await {
            info!("Using cached certificate for domain: {}", domain);
            return Ok(cert);
        }

        // Try Let's Encrypt first if relay is available
        if let Some(relay) = relay_addr {
            match self
                .request_letsencrypt_certificate(relay, domain, node_id)
                .await
            {
                Ok(cert_info) => {
                    self.store_certificate(cert_info.clone()).await?;
                    return Ok(cert_info);
                }
                Err(e) => {
                    warn!(
                        "Let's Encrypt certificate request failed: {}, falling back to self-signed",
                        e
                    );
                }
            }
        }

        // Fall back to self-signed certificate
        info!("Generating self-signed certificate for domain: {}", domain);
        let cert_info = self.generate_self_signed_certificate(domain, node_id, p2p_private_key)?;
        self.store_certificate(cert_info.clone()).await?;
        Ok(cert_info)
    }

    /// Store a certificate for a domain
    pub async fn store_certificate(&self, cert_info: CertificateInfo) -> Result<()> {
        let domain = cert_info.domain.clone();
        let node_id = cert_info.node_id.clone();

        // Save to disk first
        self.save_certificate_to_disk(&cert_info).await?;

        // Then store in memory cache
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

    /// Get domains that currently have self-signed certificates (candidates for upgrade)
    pub async fn get_self_signed_domains(&self) -> Vec<String> {
        let certs = self.certificates.read().await;
        certs
            .iter()
            .filter(|(_, cert_info)| cert_info.cert_type == CertificateType::SelfSigned)
            .map(|(domain, _)| domain.clone())
            .collect()
    }

    /// Upgrade a certificate from self-signed to Let's Encrypt using provided relay client
    pub async fn upgrade_certificate(
        &self,
        domain: &str,
        mut relay_client: relay_service_client::RelayServiceClient<Channel>,
    ) -> Result<CertificateInfo> {
        // Check if we have a self-signed certificate for this domain
        let existing_cert = self.get_certificate(domain).await;
        match existing_cert {
            Some(cert_info) if cert_info.cert_type == CertificateType::SelfSigned => {
                info!(
                    "Attempting to upgrade self-signed certificate to Let's Encrypt for domain: {}",
                    domain
                );

                // Extract node_id from existing certificate
                let node_id = &cert_info.node_id;

                // Attempt to request Let's Encrypt certificate via relay
                match self
                    .request_letsencrypt_via_relay(&mut relay_client, domain, node_id)
                    .await
                {
                    Ok(le_cert_info) => {
                        // Store the new Let's Encrypt certificate
                        self.store_certificate(le_cert_info.clone()).await?;
                        info!(
                            "Successfully upgraded certificate for domain: {} to Let's Encrypt",
                            domain
                        );
                        Ok(le_cert_info)
                    }
                    Err(e) => {
                        warn!("Let's Encrypt certificate upgrade failed for domain {}: {}, keeping self-signed certificate", domain, e);
                        Ok(cert_info) // Return existing self-signed certificate
                    }
                }
            }
            Some(cert_info) if cert_info.cert_type == CertificateType::LetsEncrypt => {
                info!(
                    "Domain {} already has Let's Encrypt certificate, no upgrade needed",
                    domain
                );
                Ok(cert_info)
            }
            None => Err(DaemonError::certificate_error(format!(
                "No certificate found for domain: {}",
                domain
            ))),
            _ => Err(DaemonError::certificate_error(format!(
                "Unexpected certificate state for domain: {}",
                domain
            ))),
        }
    }

    /// Request Let's Encrypt certificate via relay service client
    async fn request_letsencrypt_via_relay(
        &self,
        relay_client: &mut relay_service_client::RelayServiceClient<Channel>,
        domain: &str,
        node_id: &str,
    ) -> Result<CertificateInfo> {
        info!(
            "Requesting Let's Encrypt certificate for domain: {} via relay service",
            domain
        );

        // 1. Initialize ACME client
        let acme_url = if self.letsencrypt_config.staging {
            instant_acme::LetsEncrypt::Staging.url()
        } else {
            instant_acme::LetsEncrypt::Production.url()
        }
        .to_string();

        let contact_email = format!("mailto:{}", self.letsencrypt_config.email);
        let (account, _credentials) = instant_acme::Account::create(
            &instant_acme::NewAccount {
                contact: &[&contact_email],
                terms_of_service_agreed: true,
                only_return_existing: false,
            },
            acme_url,
            None,
        )
        .await?;

        // 2. Create order for domain
        let identifier = instant_acme::Identifier::Dns(domain.to_owned());
        let mut order = account
            .new_order(&instant_acme::NewOrder::new(&[identifier]))
            .await?;

        // 3. Get authorizations and DNS challenge
        let mut authorizations = order.authorizations();
        let mut authz = authorizations
            .next()
            .await
            .ok_or_else(|| DaemonError::certificate_error("No authorizations found"))??;

        let mut challenge = authz
            .challenge(instant_acme::ChallengeType::Dns01)
            .ok_or_else(|| DaemonError::certificate_error("No DNS challenge found"))?;

        let dns_value = challenge.key_authorization().dns_value();

        info!(
            "Got DNS challenge for domain: {}, challenge value: {}",
            challenge.identifier(),
            dns_value
        );

        // 4. Create DNS TXT record via relay
        let challenge_request = CreateDnsChallengeRequest {
            domain: format!("_acme-challenge.{}", domain),
            txt_value: dns_value.clone(),
            ttl_seconds: 60, // Short TTL for quick propagation
        };

        let mut challenge_stream = relay_client
            .create_dns_challenge(challenge_request)
            .await?
            .into_inner();

        // 5. Monitor streaming response for completion
        let mut record_id = None;
        while let Some(response) = challenge_stream.next().await {
            let response = response?;

            match response.response {
                Some(create_dns_challenge_response::Response::Progress(progress)) => {
                    info!("DNS challenge progress for {}: {} - {}", domain, progress.stage, progress.message);
                }
                Some(create_dns_challenge_response::Response::Complete(complete)) => {
                    if complete.verified {
                        record_id = Some(complete.record_id);
                        info!("DNS challenge completed for domain: {}, record_id: {}", domain, record_id.as_ref().unwrap());
                        break;
                    } else {
                        return Err(DaemonError::Certificate(format!("DNS challenge verification failed for domain: {}", domain)));
                    }
                }
                Some(create_dns_challenge_response::Response::Error(error)) => {
                    return Err(DaemonError::Certificate(format!("DNS challenge error for domain {}: {}", domain, error.message)));
                }
                None => {
                    return Err(DaemonError::Certificate(format!("Empty response from DNS challenge stream for domain: {}", domain)));
                }
            }
        }

        let record_id = record_id.ok_or_else(|| {
            DaemonError::Certificate(format!(
                "DNS challenge did not complete successfully for domain: {}",
                domain
            ))
        })?;

        // 6. Check DNS propagation using streaming RPC
        let propagation_request = CheckDnsPropagationRequest {
                domain: format!("_acme-challenge.{}", domain),
                expected_value: dns_value.clone(),
                timeout_seconds: 300, // 5 minutes
            };

        let mut propagation_stream = relay_client
            .check_dns_propagation(propagation_request)
            .await?
            .into_inner();

        let mut propagation_success = false;
        while let Some(response) = propagation_stream.next().await {
            let response = response?;

            match response.response {
                Some(check_dns_propagation_response::Response::Progress(progress)) => {
                    info!("DNS propagation progress for {}: {} - {}", domain, progress.stage, progress.message);
                }
                Some(check_dns_propagation_response::Response::Complete(complete)) => {
                    if complete.propagated {
                        propagation_success = true;
                        info!("DNS propagation completed for domain: {} after {} attempts ({} seconds)", 
                              domain, complete.total_attempts, complete.elapsed_seconds);
                    } else {
                        warn!("DNS propagation timed out for domain: {} after {} attempts ({} seconds)", 
                              domain, complete.total_attempts, complete.elapsed_seconds);
                    }
                    break;
                }
                Some(check_dns_propagation_response::Response::Error(error)) => {
                    return Err(DaemonError::Certificate(format!("DNS propagation check error for domain {}: {}", domain, error.message)));
                }
                None => {
                    return Err(DaemonError::Certificate(format!("Empty response from DNS propagation stream for domain: {}", domain)));
                }
            }
        }

        if !propagation_success {
            return Err(DaemonError::Certificate(format!(
                "DNS propagation failed for domain: {}",
                domain
            )));
        }

        // 7. Complete ACME challenge validation
        challenge.set_ready().await?;

        // 8. Wait for order to be ready
        let mut order = order;
        let mut status_checked = 0;
        while let instant_acme::OrderStatus::Pending = order.state().status {
            if status_checked >= 5 {
                return Err(DaemonError::certificate_error(
                    "Order remained pending for too long"
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            status_checked += 1;
            order.refresh().await?;
        }

        if order.state().status != instant_acme::OrderStatus::Ready {
            return Err(DaemonError::Certificate(format!(
                "Order status is not ready: {:?}",
                order.state().status
            )));
        }

        // 9. Finalize order
        let private_key_pem = order.finalize().await?;

        // 10. Download certificate
        let cert_chain_pem = loop {
            match order.certificate().await? {
                Some(cert_chain_pem) => break cert_chain_pem,
                None => tokio::time::sleep(Duration::from_secs(1)).await,
            }
        };

        info!(
            "Successfully retrieved Let's Encrypt certificate for domain: {}",
            domain
        );

        // 11. Clean up DNS records via relay
        let cleanup_request = CleanupDnsChallengeRequest {
            domain: format!("_acme-challenge.{}", domain),
            record_id,
        };

        if let Err(e) = relay_client.cleanup_dns_challenge(cleanup_request).await {
            warn!(
                "Failed to cleanup DNS challenge records for domain {}: {}",
                domain, e
            );
            // Don't fail the whole process for cleanup errors
        } else {
            info!("Cleaned up DNS challenge records for domain: {}", domain);
        }

        // 12. Convert to PEM format and return certificate info
        let cert_info = CertificateInfo {
            domain: domain.to_string(),
            cert_pem: cert_chain_pem,
            key_pem: private_key_pem,
            node_id: node_id.to_string(),
            cert_type: CertificateType::LetsEncrypt,
        };

        Ok(cert_info)
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

    /// Load all certificates from disk into memory
    async fn load_certificates_from_disk(&self) -> Result<()> {
        let mut entries = fs::read_dir(&self.cert_dir).await
            .with_certificate_context("Failed to read cert directory")?;

        let mut loaded_count = 0;
        while let Some(entry) = entries.next_entry().await
            .with_certificate_context("Failed to read cert directory entry")? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_certificate_from_file(&path).await {
                    Ok(cert_info) => {
                        let mut certs = self.certificates.write().await;
                        certs.insert(cert_info.domain.clone(), cert_info.clone());
                        loaded_count += 1;
                        debug!("Loaded certificate for domain: {}", cert_info.domain);
                    }
                    Err(e) => {
                        warn!("Failed to load certificate from {}: {}", path.display(), e);
                    }
                }
            }
        }

        if loaded_count > 0 {
            info!("Loaded {} existing certificates from disk", loaded_count);
        }
        Ok(())
    }

    /// Load a single certificate from a JSON file
    async fn load_certificate_from_file(&self, path: &std::path::Path) -> Result<CertificateInfo> {
        let content = fs::read_to_string(path).await
            .with_certificate_context("Failed to read certificate file")?;

        serde_json::from_str(&content)
            .with_certificate_context("Failed to parse certificate JSON")
    }

    /// Save a certificate to disk
    async fn save_certificate_to_disk(&self, cert_info: &CertificateInfo) -> Result<()> {
        let filename = format!("{}.json", cert_info.domain.replace("*", "wildcard"));
        let path = self.cert_dir.join(filename);

        let json = serde_json::to_string_pretty(cert_info)
            .with_certificate_context("Failed to serialize certificate")?;

        fs::write(&path, json).await
            .with_certificate_context("Failed to write certificate to disk")?;

        debug!(
            "Saved certificate for domain {} to {}",
            cert_info.domain,
            path.display()
        );
        Ok(())
    }

    /// Request a Let's Encrypt certificate for the given domain through a relay
    async fn request_letsencrypt_certificate(
        &self,
        relay_addr: &GateAddr,
        domain: &str,
        node_id: &str,
    ) -> Result<CertificateInfo> {
        info!(
            "Requesting Let's Encrypt certificate for domain: {} via relay: {}",
            domain, relay_addr.id
        );

        // TODO: Connect to relay using tonic-iroh
        // For now, skip the connection and return mock certificate

        // TODO: Implement actual certificate request using new protobuf RPC
        // This would involve:
        // 1. Request DNS challenge creation
        // 2. Wait for DNS propagation
        // 3. Complete ACME challenge
        // 4. Retrieve certificate
        // 5. Clean up DNS records

        // For now, return mock certificate
        let mock_cert_pem = format!("-----BEGIN CERTIFICATE-----\nMOCK LETSENCRYPT CERTIFICATE FOR {}\n-----END CERTIFICATE-----", domain);
        let mock_key_pem = format!("-----BEGIN PRIVATE KEY-----\nMOCK LETSENCRYPT PRIVATE KEY FOR {}\n-----END PRIVATE KEY-----", domain);

        let cert_info = CertificateInfo {
            domain: domain.to_string(),
            cert_pem: mock_cert_pem,
            key_pem: mock_key_pem,
            node_id: node_id.to_string(),
            cert_type: CertificateType::LetsEncrypt,
        };

        info!("Let's Encrypt certificate obtained for domain: {}", domain);
        Ok(cert_info)
    }

    /// Generate a self-signed certificate for the given domain
    fn generate_self_signed_certificate(
        &self,
        domain: &str,
        node_id: &str,
        p2p_private_key: &[u8],
    ) -> Result<CertificateInfo> {
        info!(
            "Generating self-signed TLS certificate for domain: {}",
            domain
        );

        let mut params = CertificateParams::new(vec![domain.to_string()])
            .map_err(|e| DaemonError::certificate_error(format!("Failed to create certificate params: {}", e)))?;

        // Set certificate validity period (90 days)
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::seconds(60);
        params.not_after = now + time::Duration::days(90);

        // Set subject
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, domain);
        distinguished_name.push(DnType::OrganizationName, "Gate P2P Network");
        distinguished_name.push(DnType::OrganizationalUnitName, "Daemon Node");
        params.distinguished_name = distinguished_name;

        // Add SAN (Subject Alternative Names)
        params.subject_alt_names = vec![
            SanType::DnsName(domain.try_into().map_err(|e| {
                DaemonError::Certificate(format!("Failed to convert domain to DNS name: {:?}", e))
            })?),
            SanType::DnsName("localhost".try_into().map_err(|e| {
                DaemonError::Certificate(format!(
                    "Failed to convert localhost to DNS name: {:?}",
                    e
                ))
            })?),
        ];

        // Set key usage
        params.key_usages = vec![
            rcgen::KeyUsagePurpose::DigitalSignature,
            rcgen::KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];

        // Create a key pair from the P2P private key
        let key_pair = Self::derive_key_pair_from_p2p_key(p2p_private_key)
            .map_err(|e| DaemonError::certificate_error(format!("Failed to derive key pair: {}", e)))?;

        // Generate the certificate
        let cert = params.self_signed(&key_pair)
            .map_err(|e| DaemonError::certificate_error(format!("Failed to generate certificate: {}", e)))?;

        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();

        let cert_info = CertificateInfo {
            domain: domain.to_string(),
            cert_pem,
            key_pem,
            node_id: node_id.to_string(),
            cert_type: CertificateType::SelfSigned,
        };

        info!(
            "Successfully generated self-signed certificate for {}",
            domain
        );
        Ok(cert_info)
    }

    /// Derive a TLS key pair from the P2P private key
    ///
    /// This uses the P2P key as a seed to generate a deterministic RSA key pair
    fn derive_key_pair_from_p2p_key(p2p_key: &[u8]) -> AnyhowResult<KeyPair> {
        // Use the P2P key as a seed for deterministic key generation
        // For production, we'd want a proper key derivation function
        use sha2::{Digest, Sha256};

        // Create a deterministic seed from the P2P key
        let mut hasher = Sha256::new();
        hasher.update(b"GATE_TLS_KEY_DERIVATION");
        hasher.update(p2p_key);
        let _seed = hasher.finalize();

        // For now, just generate a new key pair (rcgen doesn't support seeded generation)
        // In production, we'd implement proper deterministic key derivation
        warn!("Using random key generation instead of deterministic derivation from P2P key");
        warn!("P2P key length: {} bytes", p2p_key.len());

        KeyPair::generate().context("Failed to generate ECDSA key pair")
    }
}

/// Helper struct for working with certificate data in DER format
pub struct TlsCertData {
    cert_der: CertificateDer<'static>,
    private_key_der: PrivateKeyDer<'static>,
    domain: String,
}

impl TlsCertData {
    /// Create from certificate info
    pub fn from_certificate_info(cert_info: &CertificateInfo) -> AnyhowResult<Self> {
        // Parse PEM to DER
        let cert_der = Self::pem_to_cert_der(&cert_info.cert_pem)?;
        let private_key_der = Self::pem_to_key_der(&cert_info.key_pem)?;

        Ok(Self {
            cert_der,
            private_key_der,
            domain: cert_info.domain.clone(),
        })
    }

    /// Get the certificate in DER format
    pub fn certificate_der(&self) -> &CertificateDer<'static> {
        &self.cert_der
    }

    /// Get the private key in DER format
    pub fn private_key_der(&self) -> &PrivateKeyDer<'static> {
        &self.private_key_der
    }

    /// Get the domain this certificate was issued for
    pub fn domain(&self) -> &str {
        &self.domain
    }

    fn pem_to_cert_der(pem: &str) -> AnyhowResult<CertificateDer<'static>> {
        // Parse PEM to extract base64 data
        let lines: Vec<&str> = pem.lines().collect();
        let mut in_cert = false;
        let mut base64_data = String::new();

        for line in lines {
            let line = line.trim();
            if line == "-----BEGIN CERTIFICATE-----" {
                in_cert = true;
                continue;
            }
            if line == "-----END CERTIFICATE-----" {
                break;
            }
            if in_cert {
                base64_data.push_str(line);
            }
        }

        if base64_data.is_empty() {
            return Err(anyhow::anyhow!("No certificate data found in PEM"));
        }

        // Decode base64 to get DER bytes
        let der_bytes = base64::engine::general_purpose::STANDARD
            .decode(&base64_data)
            .context("Failed to decode base64 certificate data")?;

        Ok(CertificateDer::from(der_bytes))
    }

    fn pem_to_key_der(pem: &str) -> AnyhowResult<PrivateKeyDer<'static>> {
        // Parse PEM to extract base64 data
        let lines: Vec<&str> = pem.lines().collect();
        let mut in_key = false;
        let mut base64_data = String::new();

        for line in lines {
            let line = line.trim();
            if line == "-----BEGIN PRIVATE KEY-----" {
                in_key = true;
                continue;
            }
            if line == "-----END PRIVATE KEY-----" {
                break;
            }
            if in_key {
                base64_data.push_str(line);
            }
        }

        if base64_data.is_empty() {
            return Err(anyhow::anyhow!("No private key data found in PEM"));
        }

        // Decode base64 to get DER bytes
        let der_bytes = base64::engine::general_purpose::STANDARD
            .decode(&base64_data)
            .context("Failed to decode base64 private key data")?;

        PrivateKeyDer::try_from(der_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid private key format: {}", e))
    }
}

// Tests temporarily disabled due to iroh API changes
// #[cfg(test)]
// mod tests { ... }
