//! LetsEncrypt ACME certificate manager

use crate::{DaemonError, Result};
use acme_lib::{persist::FilePersist, Account, Certificate, Directory, DirectoryUrl};
use hellas_gate_core::GateId;
use hellas_gate_p2p::P2PSession;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, info, warn};

/// Configuration for LetsEncrypt ACME
#[derive(Debug, Clone)]
pub struct LetsEncryptConfig {
    /// Directory to store certificates and account keys
    pub cert_dir: PathBuf,
    /// Email address for ACME account registration
    pub email: String,
    /// Use staging environment (for testing)
    pub staging: bool,
    /// Domains to request certificates for
    pub domains: Vec<String>,
}

/// LetsEncrypt certificate manager
pub struct LetsEncryptManager {
    config: LetsEncryptConfig,
    directory: Directory<FilePersist>,
    account: Option<Account<FilePersist>>,
    certificate: Option<Certificate>,
}

/// Challenge solver trait for ACME challenges
pub trait ChallengeResolver: Send + Sync {
    /// Solve an HTTP-01 challenge
    /// Returns the challenge response that should be served at /.well-known/acme-challenge/{token}
    fn solve_http01_challenge(
        &self,
        token: &str,
        key_authorization: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Clean up after challenge completion
    fn cleanup_http01_challenge(
        &self,
        token: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Solve a DNS-01 challenge
    /// Should create a TXT record at _acme-challenge.{domain} with the given value
    fn solve_dns01_challenge(
        &self,
        domain: &str,
        txt_value: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Clean up DNS-01 challenge
    fn cleanup_dns01_challenge(
        &self,
        domain: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

use std::future::Future;
use std::pin::Pin;

impl LetsEncryptManager {
    /// Create a new LetsEncrypt manager
    ///
    /// # Errors
    ///
    /// Returns an error if the ACME directory cannot be accessed or configuration is invalid
    pub async fn new(config: LetsEncryptConfig) -> Result<Self> {
        info!("Initializing LetsEncrypt manager");

        // Ensure certificate directory exists
        fs::create_dir_all(&config.cert_dir).await.map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to create cert directory: {e}"))
        })?;

        // Get the appropriate ACME directory
        let directory_url = if config.staging {
            DirectoryUrl::LetsEncryptStaging
        } else {
            DirectoryUrl::LetsEncrypt
        };

        // Create file persistence for ACME data
        let persist = FilePersist::new(&config.cert_dir);

        let directory = Directory::from_url(persist, directory_url).map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to connect to ACME directory: {e}"))
        })?;

        info!(
            "Connected to {} ACME directory",
            if config.staging {
                "staging"
            } else {
                "production"
            }
        );

        Ok(Self {
            config,
            directory,
            account: None,
            certificate: None,
        })
    }

    /// Initialize or load ACME account
    ///
    /// # Errors
    ///
    /// Returns an error if account creation or loading fails
    pub async fn initialize_account(&mut self) -> Result<()> {
        let account_key_path = self.config.cert_dir.join("account.key");

        let account = if account_key_path.exists() {
            info!("Loading existing ACME account");
            // TODO: Load existing account from file
            // For now, create a new account each time
            self.create_new_account().await?
        } else {
            info!("Creating new ACME account");
            let account = self.create_new_account().await?;

            // TODO: Save account key to file
            self.save_account_key(&account, &account_key_path).await?;

            account
        };

        self.account = Some(account);
        info!("ACME account initialized successfully");
        Ok(())
    }

    /// Create a new ACME account
    async fn create_new_account(&self) -> Result<Account<FilePersist>> {
        info!("Creating new ACME account for email: {}", self.config.email);

        let account = self.directory.account(&self.config.email).map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to create ACME account: {e}"))
        })?;

        info!("ACME account created successfully");
        Ok(account)
    }

    /// Save account key to file
    async fn save_account_key(&self, account: &Account<FilePersist>, path: &PathBuf) -> Result<()> {
        debug!("Saving account key to: {:?}", path);

        // Get the account private key in PEM format
        let private_key_pem = account.acme_private_key_pem();

        // Write the private key to file
        fs::write(path, private_key_pem).await.map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to write account key file: {e}"))
        })?;

        info!("Account key saved to: {:?}", path);
        Ok(())
    }

    /// Request a new certificate for the configured domains
    ///
    /// # Errors
    ///
    /// Returns an error if certificate request fails
    pub async fn request_certificate<R>(&mut self, challenge_resolver: Arc<R>) -> Result<()>
    where
        R: ChallengeResolver + 'static,
    {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| DaemonError::Other(anyhow::anyhow!("ACME account not initialized")))?;

        info!(
            "Requesting certificate for domains: {:?}",
            self.config.domains
        );

        // 1. Create a new order for the domains
        let order = account
            .new_order(&self.config.domains[0], &[])
            .map_err(|e| DaemonError::Other(anyhow::anyhow!("Failed to create ACME order: {e}")))?;

        info!("Created ACME order, processing authorizations");

        // 2. Get authorizations for each domain
        let authorizations = order.authorizations().map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to get authorizations: {e}"))
        })?;

        // 3. Solve challenges using the provided resolver
        for auth in &authorizations {
            let domain = auth.domain_name();
            info!("Processing authorization for domain: {}", domain);

            // Get the DNS-01 challenge
            let challenge = auth.dns_challenge();

            let txt_value = challenge.dns_proof();

            // Solve the challenge using the resolver
            challenge_resolver
                .solve_dns01_challenge(domain, &txt_value)
                .await?;

            info!("DNS challenge solved for domain: {}, validating...", domain);

            // Validate the challenge (delay in milliseconds)
            challenge.validate(30000).map_err(|e| {
                DaemonError::Other(anyhow::anyhow!(
                    "Challenge validation failed for domain {}: {e}",
                    domain
                ))
            })?;

            info!("Challenge validated successfully for domain: {}", domain);

            // Clean up the challenge
            challenge_resolver.cleanup_dns01_challenge(domain).await?;
        }

        // 4. Check if validations are complete and get CsrOrder
        info!("Confirming domain validations");
        let csr_order = order.confirm_validations().ok_or_else(|| {
            DaemonError::Other(anyhow::anyhow!("Domain validations not complete"))
        })?;

        info!("Finalizing certificate order");

        // Generate a private key for the certificate (RSA 2048-bit)
        let rsa_key = openssl::rsa::Rsa::generate(2048)
            .map_err(|e| DaemonError::Other(anyhow::anyhow!("Failed to generate RSA key: {e}")))?;
        let private_key = openssl::pkey::PKey::from_rsa(rsa_key).map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to create PKey from RSA: {e}"))
        })?;
        let private_key_pem = private_key.private_key_to_pem_pkcs8().map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to convert private key to PEM: {e}"))
        })?;
        let private_key_pem_str = std::str::from_utf8(&private_key_pem).map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to convert PEM to string: {e}"))
        })?;

        // Finalize the order with the private key (5 second delay for polling)
        let cert_order = csr_order.finalize(private_key_pem_str, 5000).map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to finalize certificate order: {e}"))
        })?;

        info!("Downloading certificate");

        // Download and save the certificate
        cert_order.download_and_save_cert().map_err(|e| {
            DaemonError::Other(anyhow::anyhow!("Failed to download certificate: {e}"))
        })?;

        info!("Certificate obtained and saved successfully");

        info!("Certificate request completed successfully");
        Ok(())
    }

    /// Check if current certificate needs renewal
    ///
    /// # Errors
    ///
    /// Returns an error if certificate status cannot be determined
    pub async fn needs_renewal(&self) -> Result<bool> {
        // TODO: Implement certificate expiration checking
        // This should check if the certificate exists and when it expires
        // Return true if certificate doesn't exist or expires within 30 days

        warn!("Certificate renewal check not yet implemented - returning true");
        Ok(true)
    }

    /// Get the current certificate in DER format
    ///
    /// # Errors
    ///
    /// Returns an error if certificate is not available
    pub fn certificate_der(&self) -> Result<CertificateDer<'static>> {
        // TODO: Implement certificate retrieval
        // This should return the current certificate in DER format for use with rustls
        unimplemented!("Certificate DER retrieval not yet implemented")
    }

    /// Get the current private key in DER format
    ///
    /// # Errors
    ///
    /// Returns an error if private key is not available
    pub fn private_key_der(&self) -> Result<PrivateKeyDer<'static>> {
        // TODO: Implement private key retrieval
        // This should return the current private key in DER format for use with rustls
        unimplemented!("Private key DER retrieval not yet implemented")
    }

    /// Get the domains this certificate is valid for
    pub fn domains(&self) -> &[String] {
        &self.config.domains
    }

    /// Check if certificate is expiring soon (within 30 days)
    pub fn is_expiring_soon(&self) -> bool {
        // TODO: Implement expiration checking
        // For now, always return false since we don't have certificate loading implemented
        false
    }

    /// Get certificate expiration information
    pub fn expiration_info(&self) -> String {
        // TODO: Implement certificate info retrieval
        "Certificate info not yet implemented".to_string()
    }
}

/// Default HTTP-01 challenge resolver that stores challenges in memory
pub struct DefaultHttpChallengeResolver {
    challenges: Arc<tokio::sync::RwLock<std::collections::HashMap<String, String>>>,
}

impl DefaultHttpChallengeResolver {
    /// Create a new default HTTP challenge resolver
    pub fn new() -> Self {
        Self {
            challenges: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Get the challenge response for a given token
    pub async fn get_challenge_response(&self, token: &str) -> Option<String> {
        let challenges = self.challenges.read().await;
        challenges.get(token).cloned()
    }

    /// Get all current challenge tokens (for debugging)
    pub async fn list_challenges(&self) -> Vec<String> {
        let challenges = self.challenges.read().await;
        challenges.keys().cloned().collect()
    }
}

impl Default for DefaultHttpChallengeResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ChallengeResolver for DefaultHttpChallengeResolver {
    fn solve_http01_challenge(
        &self,
        token: &str,
        key_authorization: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let challenges = self.challenges.clone();
        let token = token.to_string();
        let key_auth = key_authorization.to_string();

        Box::pin(async move {
            debug!(
                "Storing HTTP-01 challenge: token={}, key_auth={}",
                token, key_auth
            );
            let mut challenges = challenges.write().await;
            challenges.insert(token, key_auth);
            Ok(())
        })
    }

    fn cleanup_http01_challenge(
        &self,
        token: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let challenges = self.challenges.clone();
        let token = token.to_string();

        Box::pin(async move {
            debug!("Cleaning up HTTP-01 challenge: token={}", token);
            let mut challenges = challenges.write().await;
            challenges.remove(&token);
            Ok(())
        })
    }

    fn solve_dns01_challenge(
        &self,
        domain: &str,
        txt_value: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let domain = domain.to_string();
        let txt_value = txt_value.to_string();

        Box::pin(async move {
            warn!(
                "DNS-01 challenge not implemented: domain={}, txt_value={}",
                domain, txt_value
            );
            Err(DaemonError::Other(anyhow::anyhow!(
                "DNS-01 challenge not yet implemented"
            )))
        })
    }

    fn cleanup_dns01_challenge(
        &self,
        domain: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let domain = domain.to_string();

        Box::pin(async move {
            warn!(
                "DNS-01 challenge cleanup not implemented: domain={}",
                domain
            );
            Ok(()) // Don't fail on cleanup
        })
    }
}

/// P2P-based DNS challenge resolver that communicates with relay nodes
pub struct P2PDnsChallengeResolver {
    p2p_session: Arc<P2PSession>,
    relay_peer_id: GateId,
}

impl P2PDnsChallengeResolver {
    /// Create a new P2P DNS challenge resolver
    pub fn new(p2p_session: Arc<P2PSession>, relay_peer_id: GateId) -> Self {
        Self {
            p2p_session,
            relay_peer_id,
        }
    }
}

impl ChallengeResolver for P2PDnsChallengeResolver {
    fn solve_http01_challenge(
        &self,
        _token: &str,
        _key_authorization: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            warn!("HTTP-01 challenge not supported by P2P DNS resolver");
            Err(DaemonError::Other(anyhow::anyhow!(
                "HTTP-01 challenge not supported by P2P DNS resolver"
            )))
        })
    }

    fn cleanup_http01_challenge(
        &self,
        _token: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            Ok(()) // No-op for unsupported challenge type
        })
    }

    fn solve_dns01_challenge(
        &self,
        domain: &str,
        txt_value: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let p2p_session = self.p2p_session.clone();
        let relay_peer_id = self.relay_peer_id;
        let domain = domain.to_string();
        let txt_value = txt_value.to_string();

        Box::pin(async move {
            info!(
                "Requesting DNS-01 challenge creation via P2P: domain={}, txt_value={}",
                domain, txt_value
            );

            // Send DNS challenge create request to relay and wait for confirmation
            match p2p_session
                .request_dns_challenge_create(relay_peer_id, domain.clone(), txt_value.clone())
                .await
            {
                Ok(request_id) => {
                    info!(
                        "DNS challenge created successfully for domain: {} (request_id: {})",
                        domain, request_id
                    );

                    // Wait a bit for DNS propagation before returning
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

                    Ok(())
                }
                Err(e) => {
                    warn!(
                        "Failed to create DNS challenge for domain {}: {}",
                        domain, e
                    );
                    Err(DaemonError::Other(anyhow::anyhow!(
                        "Failed to create DNS challenge: {}",
                        e
                    )))
                }
            }
        })
    }

    fn cleanup_dns01_challenge(
        &self,
        domain: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let p2p_session = self.p2p_session.clone();
        let relay_peer_id = self.relay_peer_id;
        let domain = domain.to_string();

        Box::pin(async move {
            info!(
                "Requesting DNS-01 challenge cleanup via P2P: domain={}",
                domain
            );

            // Send DNS challenge cleanup request to relay and wait for confirmation
            match p2p_session
                .request_dns_challenge_cleanup(relay_peer_id, domain.clone())
                .await
            {
                Ok(request_id) => {
                    info!(
                        "DNS challenge cleaned up successfully for domain: {} (request_id: {})",
                        domain, request_id
                    );
                    Ok(())
                }
                Err(e) => {
                    warn!(
                        "Failed to cleanup DNS challenge for domain {}: {}",
                        domain, e
                    );
                    // Don't fail on cleanup errors - ACME can continue
                    Ok(())
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_letsencrypt_config() {
        let config = LetsEncryptConfig {
            cert_dir: PathBuf::from("/tmp/test-certs"),
            email: "test@example.com".to_string(),
            staging: true,
            domains: vec!["test.example.com".to_string()],
        };

        assert_eq!(config.email, "test@example.com");
        assert!(config.staging);
        assert_eq!(config.domains.len(), 1);
    }

    #[tokio::test]
    async fn test_challenge_resolver() {
        let resolver = DefaultHttpChallengeResolver::new();

        let token = "test_token";
        let key_auth = "test_key_authorization";

        // Test storing challenge
        resolver
            .solve_http01_challenge(token, key_auth)
            .await
            .unwrap();

        // Test retrieving challenge
        let response = resolver.get_challenge_response(token).await;
        assert_eq!(response, Some(key_auth.to_string()));

        // Test cleanup
        resolver.cleanup_http01_challenge(token).await.unwrap();
        let response = resolver.get_challenge_response(token).await;
        assert_eq!(response, None);
    }
}
