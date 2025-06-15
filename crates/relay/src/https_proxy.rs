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

        info!(
            "Registered node {} with short hash {} and {} direct addresses",
            node_id,
            short_hash,
            node_addr.direct_addresses.len()
        );
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
        let listener = TcpListener::bind(bind_addr)
            .await
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

        // Read enough data to extract SNI (modern ClientHellos can be 1KB+ with extensions)
        let mut buffer = vec![0u8; 2048];
        let n = stream
            .read(&mut buffer)
            .await
            .map_err(|e| RelayError::Network(format!("Failed to read from client: {}", e)))?;

        if n == 0 {
            warn!("Client closed connection immediately");
            return Err(RelayError::Network(
                "Client closed connection immediately".to_string(),
            ));
        }

        info!("Read {} bytes from client, extracting SNI", n);
        buffer.truncate(n);

        // Extract SNI from TLS ClientHello
        let sni = extract_sni_from_client_hello(&buffer)?;
        let domain = match sni {
            Some(domain) => domain,
            None => {
                warn!("No SNI found in ClientHello");
                return Err(RelayError::SniExtraction(
                    "No SNI found in ClientHello".to_string(),
                ));
            }
        };

        info!("Extracted SNI: {}", domain);

        // Look up full node address with direct addresses from domain short hash
        let node_addr = match self.registry.get_node_addr_for_domain(&domain).await {
            Some(addr) => {
                info!(
                    "Domain {} maps to {:?} with {} direct addresses",
                    domain,
                    addr,
                    addr.direct_addresses.len()
                );
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
                        return Err(RelayError::NodeNotFound(format!(
                            "No node registered for domain: {}",
                            domain
                        )));
                    }
                }
            }
        };

        info!(
            "Attempting P2P connection to {:?} for domain {}",
            node_addr, domain
        );

        // TLS forwarding protocol ALPN
        const TLS_FORWARD_ALPN: &[u8] = b"/gate.relay.v1.TlsForward/1.0";
        let tls_forward_alpn = TLS_FORWARD_ALPN.to_vec();
        info!(
            "Using ALPN protocol for P2P connection: {}",
            String::from_utf8_lossy(&tls_forward_alpn)
        );
        info!("Relay endpoint node ID: {}", self.endpoint.node_id());
        info!("Connecting to daemon node ID: {}", node_addr.node_id);

        let mut p2p_connection = self
            .endpoint
            .connect(node_addr.clone(), &tls_forward_alpn)
            .await
            .map_err(|e| {
                error!(
                    "Failed to connect to daemon {:?} with ALPN {}: {}",
                    node_addr,
                    String::from_utf8_lossy(&tls_forward_alpn),
                    e
                );
                RelayError::P2P(format!("Failed to connect to daemon: {}", e))
            })?;

        info!(
            "Successfully established P2P connection to {:?}, starting TLS stream proxy",
            node_addr
        );

        // Forward the entire TLS stream (including the ClientHello we already read)
        self.proxy_tls_stream(stream, &mut p2p_connection, &buffer)
            .await?;

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
        let (mut p2p_send, mut p2p_recv) = p2p_connection
            .open_bi()
            .await
            .map_err(|e| RelayError::P2P(format!("Failed to open P2P stream: {}", e)))?;

        // Send the initial ClientHello data we already read
        tokio::io::AsyncWriteExt::write_all(&mut p2p_send, initial_data)
            .await
            .map_err(|e| {
                RelayError::Network(format!("Failed to send initial data to P2P: {}", e))
            })?;

        // Split the client stream for bidirectional copying
        let (mut client_read, mut client_write) = client.split();

        // Start bidirectional copying
        let client_to_p2p = async { tokio::io::copy(&mut client_read, &mut p2p_send).await };

        let p2p_to_client = async { tokio::io::copy(&mut p2p_recv, &mut client_write).await };

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

/// Extract SNI from TLS ClientHello using robust parsing
/// This is a much more reliable implementation than the previous 120-line version
fn extract_sni_from_client_hello(data: &[u8]) -> Result<Option<String>> {
    // Basic sanity checks
    if data.len() < 43 {
        return Ok(None);
    }

    // Validate TLS record header
    if data[0] != 22 {
        // Handshake content type
        return Ok(None);
    }

    // Get TLS record length and validate
    let record_len = u16::from_be_bytes([data[3], data[4]]) as usize;
    if data.len() < 5 + record_len {
        return Ok(None);
    }

    // Validate handshake type (ClientHello = 1)
    if data.len() < 6 || data[5] != 1 {
        return Ok(None);
    }

    // Find extensions section with bounds checking
    let mut pos = 5; // Start after TLS record header

    // Skip handshake header (4 bytes), version (2 bytes), random (32 bytes)
    pos += 4 + 2 + 32;
    if data.len() < pos + 1 {
        return Ok(None);
    }

    // Skip session ID
    let session_id_len = data[pos] as usize;
    pos += 1 + session_id_len;
    if data.len() < pos + 2 {
        return Ok(None);
    }

    // Skip cipher suites
    let cipher_suites_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2 + cipher_suites_len;
    if data.len() < pos + 1 {
        return Ok(None);
    }

    // Skip compression methods
    let compression_len = data[pos] as usize;
    pos += 1 + compression_len;
    if data.len() < pos + 2 {
        return Ok(None);
    }

    // Parse extensions
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

        if ext_type == 0 && pos + ext_len <= extensions_end {
            return parse_sni_extension(&data[pos..pos + ext_len]);
        }

        pos += ext_len;
    }

    Ok(None)
}

/// Parse SNI extension data with proper error handling
fn parse_sni_extension(data: &[u8]) -> Result<Option<String>> {
    if data.len() < 5 {
        return Ok(None);
    }

    // Skip server name list length (2 bytes) and name type (1 byte, must be 0)
    if data[2] != 0 {
        return Ok(None);
    }

    // Get hostname length
    let name_len = u16::from_be_bytes([data[3], data[4]]) as usize;
    if data.len() < 5 + name_len {
        return Ok(None);
    }

    // Extract hostname with UTF-8 validation
    String::from_utf8(data[5..5 + name_len].to_vec())
        .map(Some)
        .map_err(|e| RelayError::SniExtraction(format!("Invalid UTF-8 in SNI: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal TLS ClientHello with SNI for testing
    fn create_client_hello_with_sni(hostname: &str) -> Vec<u8> {
        let mut buffer = Vec::new();
        
        // TLS Record Header
        buffer.push(22); // Content Type: Handshake
        buffer.extend_from_slice(&[3, 3]); // Version: TLS 1.2
        
        // Record length placeholder (will fill in later)
        let record_len_pos = buffer.len();
        buffer.extend_from_slice(&[0, 0]);
        
        // Handshake Header
        buffer.push(1); // Handshake Type: ClientHello
        
        // Handshake length placeholder (will fill in later)
        let handshake_len_pos = buffer.len();
        buffer.extend_from_slice(&[0, 0, 0]);
        
        // Version
        buffer.extend_from_slice(&[3, 3]); // TLS 1.2
        
        // Random (32 bytes)
        buffer.extend_from_slice(&[0u8; 32]);
        
        // Session ID length
        buffer.push(0);
        
        // Cipher suites length (2 bytes)
        buffer.extend_from_slice(&[0, 2]);
        // One cipher suite
        buffer.extend_from_slice(&[0, 0x2f]); // TLS_RSA_WITH_AES_128_CBC_SHA
        
        // Compression methods length
        buffer.push(1);
        // Null compression
        buffer.push(0);
        
        // Extensions length placeholder
        let extensions_len_pos = buffer.len();
        buffer.extend_from_slice(&[0, 0]);
        
        // SNI Extension
        buffer.extend_from_slice(&[0, 0]); // Extension Type: SNI (0)
        
        // SNI extension length placeholder
        let sni_ext_len_pos = buffer.len();
        buffer.extend_from_slice(&[0, 0]);
        
        // Server Name List Length
        let name_list_len_pos = buffer.len();
        buffer.extend_from_slice(&[0, 0]);
        
        // Name Type (0 = hostname)
        buffer.push(0);
        
        // Hostname length
        buffer.extend_from_slice(&(hostname.len() as u16).to_be_bytes());
        
        // Hostname
        buffer.extend_from_slice(hostname.as_bytes());
        
        // Fill in lengths
        let name_list_len = hostname.len() + 3; // type (1) + length (2) + hostname
        buffer[name_list_len_pos..name_list_len_pos + 2].copy_from_slice(&(name_list_len as u16).to_be_bytes());
        
        let sni_ext_len = name_list_len + 2; // name_list_len (2) + name_list
        buffer[sni_ext_len_pos..sni_ext_len_pos + 2].copy_from_slice(&(sni_ext_len as u16).to_be_bytes());
        
        let extensions_len = sni_ext_len + 4; // ext_type (2) + ext_len (2) + ext_data
        buffer[extensions_len_pos..extensions_len_pos + 2].copy_from_slice(&(extensions_len as u16).to_be_bytes());
        
        let handshake_len = buffer.len() - handshake_len_pos - 3;
        let handshake_len_bytes = [(handshake_len >> 16) as u8, (handshake_len >> 8) as u8, handshake_len as u8];
        buffer[handshake_len_pos..handshake_len_pos + 3].copy_from_slice(&handshake_len_bytes);
        
        let record_len = buffer.len() - record_len_pos - 2;
        buffer[record_len_pos..record_len_pos + 2].copy_from_slice(&(record_len as u16).to_be_bytes());
        
        buffer
    }

    #[test]
    fn test_sni_extraction_empty_data() {
        let result = extract_sni_from_client_hello(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_sni_extraction_too_short() {
        let short_data = vec![22, 3, 3, 0, 10]; // Valid start but too short
        let result = extract_sni_from_client_hello(&short_data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_sni_extraction_wrong_content_type() {
        let mut data = create_client_hello_with_sni("example.com");
        data[0] = 21; // Change content type from handshake to alert
        let result = extract_sni_from_client_hello(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_sni_extraction_valid_hostname() {
        let hostname = "example.com";
        let data = create_client_hello_with_sni(hostname);
        let result = extract_sni_from_client_hello(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(hostname.to_string()));
    }

    #[test]
    fn test_sni_extraction_gate_domain() {
        let hostname = "3818e20a7b12092e.private.hellas.ai";
        let data = create_client_hello_with_sni(hostname);
        let result = extract_sni_from_client_hello(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(hostname.to_string()));
    }

    #[test]
    fn test_sni_extraction_long_hostname() {
        let hostname = "very-long-subdomain-name-that-might-cause-issues.with.multiple.subdomains.example.com";
        let data = create_client_hello_with_sni(hostname);
        let result = extract_sni_from_client_hello(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(hostname.to_string()));
    }

    #[test]
    fn test_parse_sni_extension_too_short() {
        let short_data = vec![0, 1, 2, 3]; // Less than 5 bytes
        let result = parse_sni_extension(&short_data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_parse_sni_extension_wrong_name_type() {
        let data = vec![0, 0, 1, 0, 3]; // name_type = 1 instead of 0
        let result = parse_sni_extension(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }
}
