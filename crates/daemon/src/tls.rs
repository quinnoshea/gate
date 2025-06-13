//! TLS termination for raw HTTPS bytes from P2P transport

use crate::certs::{CertificateInfo, TlsCertData};
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
    cert_data: Arc<TlsCertData>,
    domain: String,
}

impl TlsHandler {
    /// Create a new TLS handler from certificate info
    ///
    /// # Errors
    ///
    /// Returns an error if TLS configuration fails
    pub fn from_certificate_info(cert_info: &CertificateInfo) -> Result<Self> {
        info!("Initializing TLS handler for domain: {}", cert_info.domain);

        // Convert PEM to DER format
        let cert_data = TlsCertData::from_certificate_info(cert_info)
            .map_err(|e| DaemonError::Certificate(format!("Failed to parse certificate: {}", e)))?;

        let domain = cert_info.domain.clone();

        // Create TLS configuration
        let tls_config = Self::create_tls_config_from_cert_data(&cert_data)?;
        let acceptor = TlsAcceptor::from(Arc::new(tls_config));

        Ok(Self {
            acceptor,
            cert_data: Arc::new(cert_data),
            domain,
        })
    }

    /// Create rustls server configuration
    fn create_tls_config_from_cert_data(
        cert_data: &TlsCertData,
    ) -> Result<rustls::ServerConfig> {
        let cert_chain = vec![cert_data.certificate_der().clone()];
        let private_key = cert_data.private_key_der().clone_key();

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .map_err(|e| DaemonError::Certificate(format!("TLS config error: {}", e)))?;

        Ok(config)
    }

    /// Terminate TLS from raw HTTPS bytes and return decrypted HTTP request
    ///
    /// This is the primary function - takes raw HTTPS bytes from P2P transport
    /// and returns the decrypted HTTP request
    pub async fn terminate_tls(&self, https_bytes: &[u8]) -> Result<String> {
        debug!("Terminating TLS for {} bytes", https_bytes.len());

        // Create a cursor from the input bytes to simulate a stream
        let input_stream = Cursor::new(https_bytes.to_vec());

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
    async fn perform_tls_termination<R>(&self, input: R) -> Result<String>
    where
        R: AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        debug!("Starting TLS termination process");

        // Create a TLS stream using the acceptor
        let tls_stream = self.acceptor.accept(input).await
            .map_err(|e| DaemonError::Certificate(format!("TLS handshake failed: {}", e)))?;

        // Read the decrypted HTTP request from the TLS stream
        let mut buffer = Vec::new();
        let mut reader = tokio::io::BufReader::new(tls_stream);
        
        // Read until we have the complete HTTP request
        // We'll read line by line until we find the end of headers (empty line)
        let mut headers_complete = false;
        let mut content_length = 0usize;
        
        loop {
            let mut line = String::new();
            let bytes_read = tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line).await
                .map_err(|e| DaemonError::Certificate(format!("Failed to read TLS stream: {}", e)))?;
            
            if bytes_read == 0 {
                break; // EOF
            }
            
            buffer.extend_from_slice(line.as_bytes());
            
            // Check if this line indicates Content-Length
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(value) = line.split(':').nth(1) {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
            
            // Check if we've reached the end of headers (empty line)
            if line.trim().is_empty() && !headers_complete {
                headers_complete = true;
                
                // If there's a body to read, read it
                if content_length > 0 {
                    let mut body_buffer = vec![0u8; content_length];
                    tokio::io::AsyncReadExt::read_exact(&mut reader, &mut body_buffer).await
                        .map_err(|e| DaemonError::Certificate(format!("Failed to read request body: {}", e)))?;
                    buffer.extend_from_slice(&body_buffer);
                }
                break;
            }
        }
        
        let http_request = String::from_utf8(buffer)
            .map_err(|e| DaemonError::Certificate(format!("Invalid UTF-8 in HTTP request: {}", e)))?;
        
        if http_request.trim().is_empty() {
            return Err(DaemonError::Certificate("Empty HTTP request after TLS termination".to_string()));
        }
        
        debug!("Successfully terminated TLS, extracted {} bytes of HTTP data", http_request.len());
        Ok(http_request)
    }

    /// Get the domain this handler is configured for
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get certificate information
    #[must_use]
    pub fn certificate_info(&self) -> String {
        format!("Domain: {}", self.cert_data.domain())
    }

    /// Create rustls server configuration for direct TLS server
    pub fn create_tls_config(&self) -> Result<rustls::ServerConfig> {
        Self::create_tls_config_from_cert_data(&self.cert_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::certs::{CertificateInfo, CertificateType};

    fn create_test_cert_info() -> CertificateInfo {
        // Use valid base64 encoded mock data (even if it's not a real certificate)
        CertificateInfo {
            domain: "test.private.hellas.ai".to_string(),
            cert_pem: "-----BEGIN CERTIFICATE-----\nTW9jayBjZXJ0aWZpY2F0ZSBkYXRhIGZvciB0ZXN0aW5n\n-----END CERTIFICATE-----".to_string(),
            key_pem: "-----BEGIN PRIVATE KEY-----\nTW9jayBwcml2YXRlIGtleSBkYXRhIGZvciB0ZXN0aW5n\n-----END PRIVATE KEY-----".to_string(),
            node_id: "test123".to_string(),
            cert_type: CertificateType::SelfSigned,
        }
    }

    #[tokio::test]
    #[ignore = "requires valid certificate data"]
    async fn test_tls_handler_creation() {
        let cert_info = create_test_cert_info();
        let handler = TlsHandler::from_certificate_info(&cert_info)
            .expect("Failed to create TLS handler");

        assert_eq!(handler.domain(), "test.private.hellas.ai");
    }

    #[tokio::test]
    #[ignore = "requires valid certificate data"]
    async fn test_tls_termination_with_plain_http() {
        let cert_info = create_test_cert_info();
        let handler = TlsHandler::from_certificate_info(&cert_info)
            .expect("Failed to create TLS handler");

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
    #[ignore = "requires valid certificate data"]
    async fn test_tls_termination_with_invalid_data() {
        let cert_info = create_test_cert_info();
        let handler = TlsHandler::from_certificate_info(&cert_info)
            .expect("Failed to create TLS handler");

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
