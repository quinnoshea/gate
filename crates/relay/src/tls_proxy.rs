use crate::{
    error::{RelayError, Result},
    registry::NodeRegistry,
    sni::SniExtractor,
};
use hellas_gate_core::GateId;
use hellas_gate_p2p::{session::SniProxyHandle, P2PSession};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

/// TLS proxy that forwards raw TLS bytes between browsers and Gate nodes
pub struct TlsProxy {
    p2p_session: Arc<P2PSession>,
    node_registry: Arc<NodeRegistry>,
    sni_proxy_handle: SniProxyHandle,
}

impl TlsProxy {
    /// Create a new TLS proxy
    pub fn new(
        p2p_session: Arc<P2PSession>,
        node_registry: Arc<NodeRegistry>,
        sni_proxy_handle: SniProxyHandle,
    ) -> Self {
        Self {
            p2p_session,
            node_registry,
            sni_proxy_handle,
        }
    }

    /// Handle an incoming HTTPS connection from a browser
    pub async fn handle_connection(
        &self,
        mut client_stream: TcpStream,
        client_addr: SocketAddr,
        sni_extractor: SniExtractor,
    ) -> Result<()> {
        debug!("Handling connection from {}", client_addr);

        // Read initial TLS data to extract SNI
        let mut buffer = vec![0u8; 8192];
        let bytes_read = client_stream.read(&mut buffer).await?;

        if bytes_read == 0 {
            warn!("Client {} closed connection immediately", client_addr);
            return Ok(());
        }

        buffer.truncate(bytes_read);

        // Extract SNI to determine target node
        let domain = match sni_extractor.extract_sni(&buffer)? {
            Some(domain) => domain,
            None => {
                warn!("No SNI found in TLS handshake from {}", client_addr);
                return Err(RelayError::SniExtraction(
                    "No SNI extension found in ClientHello".to_string(),
                ));
            }
        };

        debug!("Extracted domain: {} from {}", domain, client_addr);

        // Parse node ID from domain (format: {node_id}.private.hellas.ai)
        let node_id = self.parse_node_id_from_domain(&domain)?;

        // Look up node in registry
        let node_info = self.node_registry.get_node(&node_id).await.ok_or_else(|| {
            RelayError::NodeNotFound {
                node_id: hex::encode(node_id.as_bytes()),
            }
        })?;

        info!(
            "Proxying {} -> node {}",
            client_addr,
            hex::encode(node_id.as_bytes())
        );

        // Get an idle SNI stream for the target node
        let sni_connection = self
            .sni_proxy_handle
            .get_stream_for_node(&node_id)
            .ok_or_else(|| {
                RelayError::Protocol(format!(
                    "No idle SNI streams available for node {}",
                    hex::encode(node_id.as_bytes())
                ))
            })?;

        // Split the SNI connection for bidirectional forwarding
        let (mut p2p_send, mut p2p_recv) = sni_connection.into_split();

        // Write the initial TLS data that we already read
        tokio::io::AsyncWriteExt::write_all(&mut p2p_send, &buffer).await?;

        // Split the client stream for bidirectional forwarding
        let (mut client_reader, mut client_writer) = client_stream.split();

        // Forward data in both directions concurrently
        let result: std::result::Result<(u64, u64), RelayError> = tokio::try_join!(
            // Client -> P2P (forward client data to node)
            async {
                tokio::io::copy(&mut client_reader, &mut p2p_send)
                    .await
                    .map_err(RelayError::Network)
            },
            // P2P -> Client (forward node data to client)
            async {
                tokio::io::copy(&mut p2p_recv, &mut client_writer)
                    .await
                    .map_err(RelayError::Network)
            }
        );

        match result {
            Ok((bytes_to_node, bytes_to_client)) => {
                info!(
                    "Connection {} completed: {} bytes to node, {} bytes to client",
                    client_addr, bytes_to_node, bytes_to_client
                );
            }
            Err(e) => {
                warn!("Connection {} failed during forwarding: {}", client_addr, e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Parse node ID from domain name
    fn parse_node_id_from_domain(&self, domain: &str) -> Result<GateId> {
        // Expected format: {hex_node_id}.private.hellas.ai
        let parts: Vec<&str> = domain.split('.').collect();

        if parts.len() < 4 || !domain.ends_with(".private.hellas.ai") {
            return Err(RelayError::InvalidDomain {
                domain: domain.to_string(),
            });
        }

        let node_id_hex = parts[0];

        // Decode hex node ID
        let node_id_bytes = hex::decode(node_id_hex).map_err(|e| RelayError::InvalidDomain {
            domain: format!("Invalid hex in domain {}: {}", domain, e),
        })?;

        if node_id_bytes.len() != 32 {
            return Err(RelayError::InvalidDomain {
                domain: format!("Node ID must be 32 bytes, got {}", node_id_bytes.len()),
            });
        }

        let mut node_id = [0u8; 32];
        node_id.copy_from_slice(&node_id_bytes);

        Ok(GateId::from_bytes(node_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_node_id_from_domain() {
        // Valid domain
        let node_id_hex = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let domain = format!("{}.private.hellas.ai", node_id_hex);

        // Create a dummy proxy for testing
        let session = hellas_gate_p2p::P2PSession::builder()
            .build()
            .await
            .unwrap();
        let proxy = TlsProxy::new(Arc::new(session), Arc::new(NodeRegistry::new()));

        let result = proxy.parse_node_id_from_domain(&domain);
        assert!(result.is_ok());

        // Invalid domain
        let result = proxy.parse_node_id_from_domain("invalid.domain.com");
        assert!(result.is_err());

        // Invalid hex
        let result = proxy.parse_node_id_from_domain("xyz.private.hellas.ai");
        assert!(result.is_err());
    }
}
