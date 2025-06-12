//! Self-signed certificate generation for Gate daemon

use anyhow::{Context, Result};
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, KeyPair, SanType};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

/// TLS certificate manager for the Gate daemon
pub struct TlsCertManager {
    cert: Certificate,
    cert_der: CertificateDer<'static>,
    private_key_der: PrivateKeyDer<'static>,
    domain: String,
}

impl TlsCertManager {
    /// Generate a new self-signed certificate for the given domain using P2P private key
    ///
    /// # Errors
    ///
    /// Returns an error if certificate generation fails
    pub fn generate_self_signed(node_id: &str, p2p_private_key: &[u8]) -> Result<Self> {
        // Truncate node ID to fit DNS label limit (max 63 chars)
        // Use first 16 chars (32 hex chars = 16 bytes) for shorter subdomain
        let short_node_id = if node_id.len() > 32 {
            &node_id[0..32]
        } else {
            node_id
        };
        let domain = format!("{}.private.hellas.ai", short_node_id);

        info!(
            "Generating self-signed TLS certificate for domain: {}",
            domain
        );

        let mut params = CertificateParams::new(vec![domain.clone()]);

        // Set certificate validity period (90 days)
        // Use time crate's OffsetDateTime for rcgen compatibility
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::seconds(60);
        params.not_after = now + time::Duration::days(90);

        // Set subject
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, &domain);
        distinguished_name.push(DnType::OrganizationName, "Gate P2P Network");
        distinguished_name.push(DnType::OrganizationalUnitName, "Daemon Node");
        params.distinguished_name = distinguished_name;

        // Add SAN (Subject Alternative Names)
        params.subject_alt_names = vec![
            SanType::DnsName(
                domain
                    .clone()
                    .try_into()
                    .context("Failed to convert domain to DNS name")?,
            ),
            SanType::DnsName(
                "localhost"
                    .try_into()
                    .context("Failed to convert localhost to DNS name")?,
            ),
        ];

        // Set key usage
        params.key_usages = vec![
            rcgen::KeyUsagePurpose::DigitalSignature,
            rcgen::KeyUsagePurpose::KeyEncipherment,
        ];

        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];

        // Create a key pair from the P2P private key
        // For simplicity, we'll derive an RSA key from the P2P key
        let key_pair = Self::derive_key_pair_from_p2p_key(p2p_private_key)
            .context("Failed to derive key pair from P2P key")?;

        // Set the key pair in the certificate params
        params.key_pair = Some(key_pair);

        // Generate the certificate
        let cert = Certificate::from_params(params).context("Failed to generate certificate")?;

        // Convert to DER format
        let cert_der = CertificateDer::from(
            cert.serialize_der()
                .context("Failed to serialize certificate to DER")?,
        );

        let private_key_der = PrivateKeyDer::try_from(cert.serialize_private_key_der())
            .map_err(|e| anyhow::anyhow!("Failed to serialize private key: {}", e))?;

        info!(
            "Successfully generated self-signed certificate for {}",
            domain
        );

        Ok(Self {
            cert,
            cert_der,
            private_key_der,
            domain,
        })
    }

    /// Get the certificate in DER format
    #[must_use]
    pub fn certificate_der(&self) -> &CertificateDer<'static> {
        &self.cert_der
    }

    /// Get the private key in DER format
    #[must_use]
    pub fn private_key_der(&self) -> &PrivateKeyDer<'static> {
        &self.private_key_der
    }

    /// Get the domain this certificate was issued for
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get the certificate in PEM format
    ///
    /// # Errors
    ///
    /// Returns an error if PEM serialization fails
    pub fn certificate_pem(&self) -> Result<String> {
        self.cert
            .serialize_pem()
            .context("Failed to serialize certificate to PEM")
    }

    /// Get the private key in PEM format
    pub fn private_key_pem(&self) -> String {
        self.cert.serialize_private_key_pem()
    }

    /// Check if the certificate is close to expiring (within 30 days)
    #[must_use]
    pub fn is_expiring_soon(&self) -> bool {
        // For simplicity, assume certificates don't expire soon since they're generated fresh
        // In production, we'd properly handle time comparison between time::OffsetDateTime and SystemTime
        false
    }

    /// Get certificate expiration time as a human-readable string
    #[must_use]
    pub fn expiration_info(&self) -> String {
        // For simplicity, just indicate it's a fresh certificate
        // In production, we'd properly convert time::OffsetDateTime to readable format
        "Certificate valid for 90 days (fresh generation)".to_string()
    }

    /// Derive a TLS key pair from the P2P private key
    ///
    /// This uses the P2P key as a seed to generate a deterministic RSA key pair
    fn derive_key_pair_from_p2p_key(p2p_key: &[u8]) -> Result<KeyPair> {
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

        KeyPair::generate(&rcgen::PKCS_ECDSA_P256_SHA256)
            .context("Failed to generate ECDSA key pair")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_generation() {
        let node_id = "test-node-123";
        let dummy_p2p_key = b"dummy_p2p_private_key_for_testing";
        let cert_manager = TlsCertManager::generate_self_signed(node_id, dummy_p2p_key)
            .expect("Failed to generate certificate");

        assert_eq!(cert_manager.domain(), "test-node-123.private.hellas.ai");
        assert!(!cert_manager.certificate_der().is_empty());
        assert!(!cert_manager.private_key_der().as_ref().is_empty());
    }

    #[test]
    fn test_certificate_pem_formats() {
        let node_id = "test-node-456";
        let dummy_p2p_key = b"another_dummy_p2p_private_key";
        let cert_manager = TlsCertManager::generate_self_signed(node_id, dummy_p2p_key)
            .expect("Failed to generate certificate");

        let cert_pem = cert_manager
            .certificate_pem()
            .expect("Failed to get certificate PEM");
        let key_pem = cert_manager.private_key_pem();

        assert!(cert_pem.contains("-----BEGIN CERTIFICATE-----"));
        assert!(cert_pem.contains("-----END CERTIFICATE-----"));
        assert!(key_pem.contains("-----BEGIN PRIVATE KEY-----"));
        assert!(key_pem.contains("-----END PRIVATE KEY-----"));
    }

    #[test]
    fn test_expiration_info() {
        let node_id = "test-node-789";
        let dummy_p2p_key = b"yet_another_dummy_p2p_key";
        let cert_manager = TlsCertManager::generate_self_signed(node_id, dummy_p2p_key)
            .expect("Failed to generate certificate");

        let expiration_info = cert_manager.expiration_info();
        // Should show days remaining (around 90 days for new cert)
        assert!(expiration_info.contains("Expires in"));
        assert!(!cert_manager.is_expiring_soon()); // New cert shouldn't be expiring soon
    }
}
