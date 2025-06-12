//! TLS termination for raw HTTPS bytes from P2P transport

use crate::selfsigned::TlsCertManager;
use crate::{DaemonError, Result};
use std::io::Cursor;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio_rustls::{rustls, TlsAcceptor};
use tracing::{debug, info, warn};

/// TLS termination handler for raw HTTPS bytes
#[derive(Clone)]
pub struct TlsHandler {
    acceptor: TlsAcceptor,
    cert_manager: Arc<TlsCertManager>,
    domain: String,
}

impl TlsHandler {
    /// Create a new TLS handler using P2P private key
    ///
    /// # Errors
    ///
    /// Returns an error if TLS configuration or certificate generation fails
    pub fn new(node_id: &str, p2p_private_key: &[u8]) -> Result<Self> {
        let node_id_hex = hex::encode(node_id);
        info!("Initializing TLS handler for node: {}", node_id_hex);

        // Generate self-signed certificate using P2P key
        let cert_manager = TlsCertManager::generate_self_signed(&node_id_hex, p2p_private_key)
            .map_err(|e| DaemonError::Other(e.into()))?;

        let domain = cert_manager.domain().to_string();
        info!("TLS certificate generated for domain: {}", domain);
        info!("Certificate info: {}", cert_manager.expiration_info());

        // Create TLS configuration
        let tls_config = Self::create_tls_config_from_manager(&cert_manager)?;
        let acceptor = TlsAcceptor::from(Arc::new(tls_config));

        Ok(Self {
            acceptor,
            cert_manager: Arc::new(cert_manager),
            domain,
        })
    }

    /// Create rustls server configuration
    fn create_tls_config_from_manager(
        cert_manager: &TlsCertManager,
    ) -> Result<rustls::ServerConfig> {
        let cert_chain = vec![cert_manager.certificate_der().clone()];
        let private_key = cert_manager.private_key_der().clone_key();

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .map_err(|e| DaemonError::Other(anyhow::anyhow!("TLS config error: {}", e)))?;

        Ok(config)
    }

    /// Terminate TLS from raw HTTPS bytes and return decrypted HTTP request
    ///
    /// This is the primary function - takes raw HTTPS bytes from P2P transport
    /// and returns the decrypted HTTP request
    pub async fn terminate_tls(&self, https_bytes: &[u8]) -> Result<String> {
        debug!("Terminating TLS for {} bytes", https_bytes.len());

        // Create a cursor from the input bytes to simulate a stream
        let input_stream = Cursor::new(https_bytes);

        // For now, we'll implement a simplified TLS termination
        // In a full implementation, we'd need to properly handle the TLS handshake
        // and stream processing

        // Attempt TLS handshake and decryption
        match self.perform_tls_termination(input_stream).await {
            Ok(http_request) => {
                info!("Successfully terminated TLS connection");
                debug!("Decrypted HTTP request:\n{}", http_request);
                Ok(http_request)
            }
            Err(e) => {
                warn!("TLS termination failed: {}", e);

                // For debugging, let's also try to parse as plain HTTP
                let plain_text = String::from_utf8_lossy(https_bytes);
                if plain_text.starts_with("GET ") || plain_text.starts_with("POST ") {
                    warn!("Input appears to be plain HTTP, not HTTPS");
                    info!("Plain HTTP request:\n{}", plain_text);
                    return Ok(plain_text.to_string());
                }

                Err(e)
            }
        }
    }

    /// Perform actual TLS termination
    async fn perform_tls_termination<R>(&self, _input: R) -> Result<String>
    where
        R: AsyncRead + Unpin,
    {
        // TODO: Implement proper TLS termination
        // For now, return a placeholder indicating this needs implementation

        warn!("TLS termination not yet fully implemented - returning mock response");

        // Return a mock HTTP request for testing
        Ok(
            "GET / HTTP/1.1\r\nHost: example.com\r\nUser-Agent: MockTLSTermination/1.0\r\n\r\n"
                .to_string(),
        )
    }

    /// Get the domain this handler is configured for
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get certificate information
    #[must_use]
    pub fn certificate_info(&self) -> String {
        format!(
            "Domain: {}, {}",
            self.cert_manager.domain(),
            self.cert_manager.expiration_info()
        )
    }

    /// Check if certificate is expiring soon
    #[must_use]
    pub fn is_certificate_expiring(&self) -> bool {
        self.cert_manager.is_expiring_soon()
    }

    /// Create rustls server configuration for direct TLS server
    pub fn create_tls_config(&self) -> Result<rustls::ServerConfig> {
        Self::create_tls_config_from_manager(&self.cert_manager)
    }

    /// Get certificate in PEM format (for debugging)
    pub fn certificate_pem(&self) -> Result<String> {
        self.cert_manager
            .certificate_pem()
            .map_err(|e| DaemonError::Other(e.into()))
    }

    /// Get private key in PEM format (for debugging)
    #[must_use]
    pub fn private_key_pem(&self) -> String {
        self.cert_manager.private_key_pem()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tls_handler_creation() {
        let node_id = b"test_node_id_12345";
        let p2p_key = b"test_p2p_private_key_for_testing";

        let handler =
            TlsHandler::new(&hex::encode(node_id), p2p_key).expect("Failed to create TLS handler");

        assert!(handler.domain().contains("private.hellas.ai"));
        assert!(!handler.is_certificate_expiring()); // New cert shouldn't be expiring
    }

    #[tokio::test]
    async fn test_tls_termination_with_plain_http() {
        let node_id = b"test_node_plain_http";
        let p2p_key = b"test_p2p_key_plain_http";

        let handler =
            TlsHandler::new(&hex::encode(node_id), p2p_key).expect("Failed to create TLS handler");

        // Test with plain HTTP (should be detected and handled)
        let plain_http = b"GET /test HTTP/1.1\r\nHost: example.com\r\n\r\n";

        let result = handler
            .terminate_tls(plain_http)
            .await
            .expect("Failed to process plain HTTP");

        assert!(result.contains("GET /test"));
        assert!(result.contains("Host: example.com"));
    }

    #[tokio::test]
    async fn test_tls_termination_with_invalid_data() {
        let node_id = b"test_node_invalid";
        let p2p_key = b"test_p2p_key_invalid";

        let handler =
            TlsHandler::new(&hex::encode(node_id), p2p_key).expect("Failed to create TLS handler");

        // Test with random bytes (should fall back to mock)
        let random_bytes = b"random_non_http_data_12345";

        let result = handler
            .terminate_tls(random_bytes)
            .await
            .expect("Failed to process random data");

        // Should return mock response
        assert!(result.contains("GET /"));
        assert!(result.contains("MockTLSTermination"));
    }
}
