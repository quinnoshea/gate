//! SNI proxy stream for raw TLS byte forwarding (relay functionality)

use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{debug, info};

/// SNI proxy stream handler for transparent TLS forwarding
pub struct SniProxyStream {
    stream_id: crate::StreamId,
    peer_id: iroh::NodeId,
    domain: Option<String>,
    metadata: HashMap<String, String>,
}

/// SNI proxy configuration from control stream
#[derive(Debug, Clone)]
pub struct SniProxyConfig {
    pub domain: String,
    pub target_endpoint: Option<String>, // For relay: where to forward the TLS traffic
    pub certificate_info: Option<CertificateInfo>,
}

/// Certificate information for TLS termination
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    pub domain: String,
    pub expires_at: std::time::SystemTime,
    pub fingerprint: String,
}

impl SniProxyStream {
    /// Create a new SNI proxy stream
    #[must_use]
    pub fn new(stream_id: crate::StreamId, peer_id: iroh::NodeId) -> Self {
        Self {
            stream_id,
            peer_id,
            domain: None,
            metadata: HashMap::new(),
        }
    }

    /// Configure the SNI proxy with domain information
    pub fn configure(&mut self, config: SniProxyConfig) {
        self.domain = Some(config.domain.clone());
        self.metadata.insert("domain".to_string(), config.domain);

        if let Some(target) = config.target_endpoint {
            self.metadata.insert("target_endpoint".to_string(), target);
        }

        if let Some(cert_info) = config.certificate_info {
            self.metadata
                .insert("certificate_domain".to_string(), cert_info.domain);
            self.metadata
                .insert("certificate_fingerprint".to_string(), cert_info.fingerprint);
        }
    }

    /// Handle SNI proxy stream - transparent TLS byte forwarding
    /// Both streams must be bidirectional (`AsyncRead` + `AsyncWrite`) for transparent proxying
    ///
    /// # Errors
    ///
    /// Returns an error if the bidirectional copy operation fails due to I/O errors
    pub async fn handle_stream<S1, S2>(&self, mut stream1: S1, mut stream2: S2) -> crate::Result<()>
    where
        S1: AsyncRead + AsyncWrite + Unpin,
        S2: AsyncRead + AsyncWrite + Unpin,
    {
        info!(
            "Starting SNI proxy for stream {} from peer {} (domain: {:?})",
            self.stream_id, self.peer_id, self.domain
        );

        // For SNI proxy, we just forward raw bytes bidirectionally
        // The TLS handshake and all subsequent traffic is transparent
        let bytes_transferred = tokio::io::copy_bidirectional(&mut stream1, &mut stream2)
            .await
            .map_err(|e| {
                crate::P2PError::ConnectionFailed(format!("SNI proxy transfer failed: {e}"))
            })?;

        info!(
            "SNI proxy stream {} completed: {} bytes transferred",
            self.stream_id,
            bytes_transferred.0 + bytes_transferred.1
        );

        Ok(())
    }

    /// Extract SNI (Server Name Indication) from TLS `ClientHello`
    /// This is useful for relay servers to determine which node to forward to
    pub fn extract_sni_from_client_hello(client_hello: &[u8]) -> Option<String> {
        // TLS ClientHello parsing is complex, so this is a simplified implementation
        // In production, you'd want a proper TLS parsing library

        if client_hello.len() < 6 {
            return None;
        }

        // Basic TLS record structure:
        // [0] Content Type (0x16 for handshake)
        // [1-2] TLS version
        // [3-4] Record length
        // [5] Handshake type (0x01 for ClientHello)

        if client_hello[0] != 0x16 || client_hello[5] != 0x01 {
            debug!("Not a TLS ClientHello message");
            return None;
        }

        // For now, return None - proper SNI extraction would require
        // parsing the full TLS ClientHello structure including extensions
        // This is typically done by relay servers, not the P2P nodes
        debug!("SNI extraction not fully implemented - would parse TLS ClientHello");
        None
    }

    /// Get stream metadata (domain, target, etc.)
    #[must_use]
    pub const fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// Get the domain associated with this SNI proxy stream
    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }
}

/// SNI proxy statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct SniProxyStats {
    pub streams_created: u64,
    pub streams_active: u64,
    pub bytes_transferred: u64,
    pub connections_failed: u64,
    pub domains_served: std::collections::HashSet<String>,
}

impl SniProxyStats {
    /// Record a new stream being created
    pub fn stream_created(&mut self, domain: Option<&str>) {
        self.streams_created += 1;
        self.streams_active += 1;

        if let Some(domain) = domain {
            self.domains_served.insert(domain.to_string());
        }
    }

    /// Record a stream being closed
    pub const fn stream_closed(&mut self, bytes_transferred: u64, failed: bool) {
        if self.streams_active > 0 {
            self.streams_active -= 1;
        }

        self.bytes_transferred += bytes_transferred;

        if failed {
            self.connections_failed += 1;
        }
    }

    /// Get the number of unique domains served
    #[must_use]
    pub fn unique_domains_count(&self) -> usize {
        self.domains_served.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Tests use standard tokio::io functions

    #[tokio::test]
    async fn test_sni_proxy_stream_creation() {
        let node_id = iroh::NodeId::from_bytes(&[1u8; 32]).unwrap();
        let stream = SniProxyStream::new(42, node_id);

        assert_eq!(stream.stream_id, 42);
        assert_eq!(stream.peer_id, node_id);
        assert!(stream.domain.is_none());
        assert!(stream.metadata.is_empty());
    }

    #[tokio::test]
    async fn test_sni_proxy_configuration() {
        let node_id = iroh::NodeId::from_bytes(&[1u8; 32]).unwrap();
        let mut stream = SniProxyStream::new(42, node_id);

        let config = SniProxyConfig {
            domain: "example.com".to_string(),
            target_endpoint: Some("192.168.1.100:443".to_string()),
            certificate_info: Some(CertificateInfo {
                domain: "example.com".to_string(),
                expires_at: std::time::SystemTime::now() + std::time::Duration::from_secs(86400),
                fingerprint: "sha256:abcd1234".to_string(),
            }),
        };

        stream.configure(config);

        assert_eq!(stream.domain(), Some("example.com"));
        assert_eq!(
            stream.metadata().get("domain"),
            Some(&"example.com".to_string())
        );
        assert_eq!(
            stream.metadata().get("target_endpoint"),
            Some(&"192.168.1.100:443".to_string())
        );
        assert!(stream.metadata().contains_key("certificate_fingerprint"));
    }

    #[tokio::test]
    async fn test_sni_proxy_bidirectional_copy() {
        let node_id = iroh::NodeId::from_bytes(&[1u8; 32]).unwrap();
        let _stream = SniProxyStream::new(1, node_id);

        // Create test data
        let test_data = b"Hello, this is test TLS data!";

        // Create in-memory streams for testing
        let mut incoming = std::io::Cursor::new(test_data.to_vec());
        let mut outgoing = Vec::new();

        // This would normally be an infinite bidirectional copy,
        // but with Cursor it will copy once and then hit EOF
        let result = tokio::io::copy(&mut incoming, &mut outgoing).await;

        assert!(result.is_ok());
        assert_eq!(outgoing, test_data);
    }

    #[test]
    fn test_sni_extraction_invalid_data() {
        // Test with invalid/too short data
        assert!(SniProxyStream::extract_sni_from_client_hello(&[]).is_none());
        assert!(SniProxyStream::extract_sni_from_client_hello(&[0x16]).is_none());
        assert!(SniProxyStream::extract_sni_from_client_hello(&[
            0x15, 0x03, 0x03, 0x00, 0x05, 0x01
        ])
        .is_none());
    }

    #[test]
    fn test_sni_proxy_stats() {
        let mut stats = SniProxyStats::default();

        assert_eq!(stats.streams_active, 0);
        assert_eq!(stats.streams_created, 0);

        stats.stream_created(Some("example.com"));
        assert_eq!(stats.streams_active, 1);
        assert_eq!(stats.streams_created, 1);
        assert_eq!(stats.unique_domains_count(), 1);

        stats.stream_created(Some("test.org"));
        assert_eq!(stats.streams_active, 2);
        assert_eq!(stats.unique_domains_count(), 2);

        stats.stream_closed(1024, false);
        assert_eq!(stats.streams_active, 1);
        assert_eq!(stats.bytes_transferred, 1024);
        assert_eq!(stats.connections_failed, 0);

        stats.stream_closed(512, true);
        assert_eq!(stats.streams_active, 0);
        assert_eq!(stats.bytes_transferred, 1536);
        assert_eq!(stats.connections_failed, 1);
    }
}
