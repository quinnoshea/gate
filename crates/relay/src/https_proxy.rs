//! HTTPS proxy that extracts SNI and forwards to P2P connections

use crate::error::{RelayError, Result};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tracing::{debug, error, info, warn};

/// Simple registry for short hash → full node ID mapping and addresses
#[derive(Default, Debug)]
pub struct ProxyRegistry {
    /// Maps short node hash (16 hex chars) → full node ID
    short_to_full: RwLock<HashMap<String, iroh::NodeId>>,
    /// Maps node ID → full node address with direct addresses
    node_addresses: RwLock<HashMap<iroh::NodeId, iroh::NodeAddr>>,
}

impl ProxyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a node ID when it connects (opportunistic registration)
    pub async fn register_node(&self, node_id: iroh::NodeId) {
        let short_hash = hex::encode(node_id.as_bytes())[..16].to_string();
        let mut registry = self.short_to_full.write().await;
        registry.insert(short_hash.clone(), node_id);
        info!("Registered node {} with short hash {}", node_id, short_hash);
    }

    /// Register a node address with full connectivity info
    pub async fn register_node_addr(&self, node_addr: iroh::NodeAddr) {
        let node_id = node_addr.node_id;
        let short_hash = hex::encode(node_id.as_bytes())[..16].to_string();
        
        // Register in both maps
        {
            let mut registry = self.short_to_full.write().await;
            registry.insert(short_hash.clone(), node_id);
        }
        {
            let mut addresses = self.node_addresses.write().await;
            addresses.insert(node_id, node_addr.clone());
        }
        
        info!("Registered node {} with short hash {} and {} direct addresses", 
              node_id, short_hash, node_addr.direct_addresses.len());
    }

    /// Look up full node ID from domain short hash
    pub async fn get_node_for_domain(&self, domain: &str) -> Option<iroh::NodeId> {
        // Extract short hash from domain (e.g., "abc123.private.hellas.ai" → "abc123")
        let short_hash = domain.split('.').next()?.to_string();
        let registry = self.short_to_full.read().await;
        registry.get(&short_hash).copied()
    }

    /// Look up full node address with direct addresses from domain short hash
    pub async fn get_node_addr_for_domain(&self, domain: &str) -> Option<iroh::NodeAddr> {
        // Extract short hash from domain (e.g., "abc123.private.hellas.ai" → "abc123")
        let short_hash = domain.split('.').next()?.to_string();
        
        // Get node ID first
        let node_id = {
            let registry = self.short_to_full.read().await;
            registry.get(&short_hash).copied()?
        };
        
        // Try to get full node address with direct addresses
        let addresses = self.node_addresses.read().await;
        addresses.get(&node_id).cloned()
    }
}

/// HTTPS proxy that handles SNI extraction and P2P forwarding
pub struct HttpsProxy {
    /// Simple registry for domain mapping
    registry: Arc<ProxyRegistry>,
    /// Iroh endpoint for P2P connections
    endpoint: iroh::Endpoint,
}

impl HttpsProxy {
    /// Create a new HTTPS proxy
    pub fn new(endpoint: iroh::Endpoint) -> Self {
        Self {
            registry: Arc::new(ProxyRegistry::new()),
            endpoint,
        }
    }

    /// Get a reference to the registry for domain registration
    pub fn registry(&self) -> Arc<ProxyRegistry> {
        self.registry.clone()
    }


    /// Start listening on port 443 for HTTPS connections
    pub async fn listen(&self, bind_addr: &str) -> Result<()> {
        let listener = TcpListener::bind(bind_addr).await
            .map_err(|e| RelayError::Network(format!("Failed to bind to {}: {}", bind_addr, e)))?;
        
        info!("HTTPS proxy listening on {}", bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New HTTPS connection from {}", addr);
                    let proxy = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = proxy.handle_connection(stream).await {
                            warn!("Failed to handle connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle a single HTTPS connection
    async fn handle_connection(&self, mut stream: TcpStream) -> Result<()> {
        info!("New HTTPS connection received, reading ClientHello for SNI extraction");
        
        // Read enough data to extract SNI (first ~512 bytes should be sufficient)
        let mut buffer = vec![0u8; 512];
        let n = stream.read(&mut buffer).await
            .map_err(|e| RelayError::Network(format!("Failed to read from client: {}", e)))?;
        
        if n == 0 {
            warn!("Client closed connection immediately");
            return Err(RelayError::Network("Client closed connection immediately".to_string()));
        }

        info!("Read {} bytes from client, extracting SNI", n);
        buffer.truncate(n);

        // Extract SNI from TLS ClientHello
        let sni = extract_sni_from_client_hello(&buffer)?;
        let domain = match sni {
            Some(domain) => domain,
            None => {
                warn!("No SNI found in ClientHello");
                return Err(RelayError::SniExtraction("No SNI found in ClientHello".to_string()));
            }
        };

        info!("Extracted SNI: {}", domain);

        // Look up full node address with direct addresses from domain short hash
        let node_addr = match self.registry.get_node_addr_for_domain(&domain).await {
            Some(addr) => {
                info!("Domain {} maps to {:?} with {} direct addresses", 
                      domain, addr, addr.direct_addresses.len());
                addr
            }
            None => {
                // Fallback: try to get just the node ID and let iroh handle discovery
                match self.registry.get_node_for_domain(&domain).await {
                    Some(node_id) => {
                        warn!("Domain {} maps to node {} but no direct addresses cached, using discovery", 
                              domain, node_id);
                        iroh::NodeAddr::new(node_id)
                    }
                    None => {
                        warn!("No node registered for domain: {}", domain);
                        return Err(RelayError::NodeNotFound(format!("No node registered for domain: {}", domain)));
                    }
                }
            }
        };
        
        info!("Attempting P2P connection to {:?} for domain {}", node_addr, domain);
        
        // TLS forwarding protocol ALPN
        const TLS_FORWARD_ALPN: &[u8] = b"/gate.relay.v1.TlsForward/1.0";
        let tls_forward_alpn = TLS_FORWARD_ALPN.to_vec();
        info!("Using ALPN protocol for P2P connection: {}", String::from_utf8_lossy(&tls_forward_alpn));
        info!("Relay endpoint node ID: {}", self.endpoint.node_id());
        info!("Connecting to daemon node ID: {}", node_addr.node_id);
        
        let mut p2p_connection = self.endpoint.connect(node_addr.clone(), &tls_forward_alpn).await
            .map_err(|e| {
                error!("Failed to connect to daemon {:?} with ALPN {}: {}", 
                       node_addr, String::from_utf8_lossy(&tls_forward_alpn), e);
                RelayError::P2P(format!("Failed to connect to daemon: {}", e))
            })?;

        info!("Successfully established P2P connection to {:?}, starting TLS stream proxy", node_addr);

        // Forward the entire TLS stream (including the ClientHello we already read)
        self.proxy_tls_stream(stream, &mut p2p_connection, &buffer).await?;

        info!("TLS stream proxy completed for domain {}", domain);
        Ok(())
    }


    /// Proxy the TLS stream between client and P2P connection
    async fn proxy_tls_stream(
        &self,
        mut client: TcpStream,
        p2p_connection: &mut iroh::endpoint::Connection,
        initial_data: &[u8],
    ) -> Result<()> {
        // Open a new stream on the P2P connection
        let (mut p2p_send, mut p2p_recv) = p2p_connection.open_bi().await
            .map_err(|e| RelayError::P2P(format!("Failed to open P2P stream: {}", e)))?;

        // Send the initial ClientHello data we already read
        tokio::io::AsyncWriteExt::write_all(&mut p2p_send, initial_data).await
            .map_err(|e| RelayError::Network(format!("Failed to send initial data to P2P: {}", e)))?;

        // Split the client stream for bidirectional copying
        let (mut client_read, mut client_write) = client.split();

        // Start bidirectional copying
        let client_to_p2p = async {
            tokio::io::copy(&mut client_read, &mut p2p_send).await
        };

        let p2p_to_client = async {
            tokio::io::copy(&mut p2p_recv, &mut client_write).await
        };

        // Run both copy operations concurrently
        tokio::select! {
            result = client_to_p2p => {
                match result {
                    Ok(bytes) => debug!("Client→P2P: {} bytes", bytes),
                    Err(e) => warn!("Client→P2P copy failed: {}", e),
                }
            }
            result = p2p_to_client => {
                match result {
                    Ok(bytes) => debug!("P2P→Client: {} bytes", bytes),
                    Err(e) => warn!("P2P→Client copy failed: {}", e),
                }
            }
        }

        debug!("TLS proxy session completed");
        Ok(())
    }
}

impl Clone for HttpsProxy {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            endpoint: self.endpoint.clone(),
        }
    }
}

/// Extract SNI from TLS ClientHello data
fn extract_sni_from_client_hello(data: &[u8]) -> Result<Option<String>> {
    if data.len() < 43 {
        return Ok(None);
    }

    // Simple SNI extraction - parse TLS record and find SNI extension
    let mut pos = 0;

    // Skip TLS record header (5 bytes)
    if data.len() < pos + 5 {
        return Ok(None);
    }
    
    let content_type = data[pos];
    if content_type != 22 { // Not a handshake record
        return Ok(None);
    }
    pos += 5;

    // Skip handshake header (4 bytes)
    if data.len() < pos + 4 {
        return Ok(None);
    }
    
    let handshake_type = data[pos];
    if handshake_type != 1 { // Not ClientHello
        return Ok(None);
    }
    pos += 4;

    // Skip ClientHello version (2 bytes) and random (32 bytes)
    if data.len() < pos + 34 {
        return Ok(None);
    }
    pos += 34;

    // Skip session ID
    if data.len() < pos + 1 {
        return Ok(None);
    }
    let session_id_len = data[pos] as usize;
    pos += 1 + session_id_len;

    // Skip cipher suites
    if data.len() < pos + 2 {
        return Ok(None);
    }
    let cipher_suites_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2 + cipher_suites_len;

    // Skip compression methods
    if data.len() < pos + 1 {
        return Ok(None);
    }
    let compression_methods_len = data[pos] as usize;
    pos += 1 + compression_methods_len;

    // Parse extensions
    if data.len() < pos + 2 {
        return Ok(None);
    }
    let extensions_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2;

    let extensions_end = pos + extensions_len;
    if data.len() < extensions_end {
        return Ok(None);
    }

    // Look for SNI extension (type 0)
    while pos + 4 <= extensions_end {
        let ext_type = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let ext_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if ext_type == 0 && pos + ext_len <= extensions_end { // SNI extension
            return parse_sni_extension(&data[pos..pos + ext_len]);
        }

        pos += ext_len;
    }

    Ok(None)
}

/// Parse the SNI extension data to extract the hostname
fn parse_sni_extension(data: &[u8]) -> Result<Option<String>> {
    if data.len() < 5 {
        return Ok(None);
    }

    let mut pos = 0;
    
    // Skip server name list length (2 bytes)
    pos += 2;

    // Parse first server name entry
    if data.len() < pos + 3 {
        return Ok(None);
    }

    let name_type = data[pos];
    if name_type != 0 { // hostname type
        return Ok(None);
    }
    pos += 1;

    let name_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2;

    if data.len() < pos + name_len {
        return Ok(None);
    }

    let hostname = String::from_utf8(data[pos..pos + name_len].to_vec())
        .map_err(|e| RelayError::SniExtraction(format!("Invalid UTF-8 in SNI: {}", e)))?;

    Ok(Some(hostname))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sni_extraction() {
        // This would contain actual TLS ClientHello bytes for testing
        // For now, just test that the function doesn't panic with empty data
        let result = extract_sni_from_client_hello(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }
}