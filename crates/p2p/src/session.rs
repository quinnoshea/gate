//! P2P session with all protocol methods

use hellas_gate_core::{GateAddr, GateId};
use iroh::{endpoint::Endpoint, NodeAddr, NodeId};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::stream::P2PStream;
use crate::{P2PError, Result};

/// Gate protocol identifier
const GATE_PROTOCOL: &[u8] = b"gate/1.0";

/// Commands for controlling P2P session
#[derive(Debug, Clone)]
pub enum P2PCommand {
    /// Shutdown the session gracefully
    Shutdown,
}

/// A pending inference request that needs a response
pub struct PendingRequest {
    payload: JsonValue,
    peer_id: NodeId,
    response_tx: tokio::sync::oneshot::Sender<JsonValue>,
}

impl PendingRequest {
    /// Get the request payload
    #[must_use]
    pub const fn payload(&self) -> &JsonValue {
        &self.payload
    }

    /// Get the peer ID that sent this request
    #[must_use]
    pub const fn peer_id(&self) -> NodeId {
        self.peer_id
    }

    /// Send a response back to the peer
    ///
    /// # Errors
    ///
    /// Returns an error if the response channel is closed
    pub fn respond(self, response: JsonValue) -> Result<()> {
        self.response_tx
            .send(response)
            .map_err(|_| P2PError::ConnectionFailed("Failed to send response".to_string()))?;
        Ok(())
    }
}

/// Convert iroh `NodeId` to `GateId`
fn node_id_to_gate_id(node_id: NodeId) -> GateId {
    GateId::from_bytes(*node_id.as_bytes())
}

/// Convert `GateId` to iroh `NodeId`
fn gate_id_to_node_id(gate_id: GateId) -> NodeId {
    NodeId::from_bytes(gate_id.as_bytes()).expect("Valid 32-byte GateId should convert to NodeId")
}

/// Convert iroh `NodeAddr` to `GateAddr`
fn node_addr_to_gate_addr(node_addr: &NodeAddr) -> GateAddr {
    // For localhost connections, prefer direct addresses
    let addr_str = node_addr.direct_addresses().next().map_or_else(
        || format!("{node_addr:?}"),
        |direct_addr| format!("{direct_addr}"),
    );

    GateAddr::new(node_id_to_gate_id(node_addr.node_id), addr_str)
}

/// Convert `GateAddr` string to iroh `NodeAddr`
fn parse_node_addr(addr_str: &str) -> Result<NodeAddr> {
    // Try to parse as just a NodeId first (hex string)
    if let Ok(gate_id) = addr_str.parse::<hellas_gate_core::GateId>() {
        let node_id = gate_id_to_node_id(gate_id);
        return Ok(NodeAddr::new(node_id));
    }

    // Try to parse as NodeId@address format
    if let Some((node_id_str, addr_part)) = addr_str.split_once('@') {
        if let Ok(gate_id) = node_id_str.parse::<hellas_gate_core::GateId>() {
            let node_id = gate_id_to_node_id(gate_id);
            let mut node_addr = NodeAddr::new(node_id);

            // Check if it's a relay URL
            if addr_part.starts_with("http") {
                if let Ok(relay) = addr_part.parse() {
                    node_addr = node_addr.with_relay_url(relay);
                }
            }
            // Check if it's a direct address (localhost:port or IP:port)
            else if let Ok(socket_addr) = addr_part.parse::<std::net::SocketAddr>() {
                node_addr = node_addr.with_direct_addresses(vec![socket_addr]);
            }

            return Ok(node_addr);
        }
    }

    Err(P2PError::Protocol(format!("Invalid node address format: {addr_str}. Expected NodeId (hex), NodeId@relay_url, or NodeId@host:port")))
}

/// Builder for P2P session
pub struct P2PSessionBuilder {
    identity: Option<iroh::SecretKey>,
    port: u16,
}

/// P2P session with all protocol methods
pub struct P2PSession {
    endpoint: Endpoint,
    node_id: NodeId,
    connections: Arc<RwLock<HashMap<NodeId, iroh::endpoint::Connection>>>,
    command_tx: mpsc::UnboundedSender<P2PCommand>,
    _connection_handler: JoinHandle<()>,
}

impl P2PSessionBuilder {
    /// Create a new builder
    #[must_use]
    pub const fn new() -> Self {
        Self {
            identity: None,
            port: 0, // Default to random port
        }
    }

    /// Set the node identity (private key)
    #[must_use]
    pub fn with_identity(mut self, identity: iroh::SecretKey) -> Self {
        self.identity = Some(identity);
        self
    }

    /// Set the port to bind to (0 for random port)
    #[must_use]
    pub const fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Generate a new random identity
    #[must_use]
    pub fn with_generated_identity(mut self) -> Self {
        let mut rng = rand::thread_rng();
        self.identity = Some(iroh::SecretKey::generate(&mut rng));
        self
    }

    /// Generate a new identity and return both the builder and the key bytes for saving
    #[must_use]
    pub fn generate_identity_with_bytes(mut self) -> (Self, [u8; 32]) {
        let mut rng = rand::thread_rng();
        let secret_key = iroh::SecretKey::generate(&mut rng);
        let key_bytes = secret_key.to_bytes();
        self.identity = Some(secret_key);
        (self, key_bytes)
    }

    /// Set the node identity from private key bytes (32 bytes)
    ///
    /// # Errors
    ///
    /// Returns an error if the private key bytes are not exactly 32 bytes
    pub fn with_private_key(mut self, key_bytes: &[u8]) -> Result<Self> {
        if key_bytes.len() != 32 {
            return Err(P2PError::Protocol(format!(
                "Private key must be exactly 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        let key_array: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| P2PError::Protocol("Failed to convert key bytes to array".to_string()))?;
        let secret_key = iroh::SecretKey::from_bytes(&key_array);
        self.identity = Some(secret_key);
        Ok(self)
    }

    /// Start the P2P session
    ///
    /// # Errors
    ///
    /// Returns an error if endpoint binding fails
    ///
    /// # Panics
    ///
    /// Panics if the bind address parsing fails
    pub async fn start(self) -> Result<(P2PSession, mpsc::UnboundedReceiver<PendingRequest>)> {
        let mut endpoint_builder = Endpoint::builder()
            .alpns(vec![GATE_PROTOCOL.to_vec()])
            .relay_mode(iroh::RelayMode::Disabled); // Disable relay for local testing

        // Only bind to specific port if non-zero, otherwise use random port
        if self.port != 0 {
            let bind_addr = format!("0.0.0.0:{}", self.port);
            endpoint_builder = endpoint_builder.bind_addr_v4(bind_addr.parse().unwrap());
        } else {
            endpoint_builder = endpoint_builder.bind_addr_v4("0.0.0.0:0".parse().unwrap());
        }

        if let Some(identity) = self.identity {
            endpoint_builder = endpoint_builder.secret_key(identity);
        }

        let endpoint = endpoint_builder
            .bind()
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to bind endpoint: {e}")))?;

        let node_id = endpoint.node_id();
        let connections = Arc::new(RwLock::new(HashMap::new()));

        // Create channels for commands and requests
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (request_tx, request_rx) = mpsc::unbounded_channel();

        // Start connection handler
        let connection_handler = Self::start_connection_handler(
            endpoint.clone(),
            Arc::clone(&connections),
            command_rx,
            request_tx,
        );

        info!("P2P session started with node ID: {node_id}");

        let session = P2PSession {
            endpoint,
            node_id,
            connections,
            command_tx,
            _connection_handler: connection_handler,
        };

        Ok((session, request_rx))
    }

    fn start_connection_handler(
        endpoint: Endpoint,
        connections: Arc<RwLock<HashMap<NodeId, iroh::endpoint::Connection>>>,
        mut command_rx: mpsc::UnboundedReceiver<P2PCommand>,
        request_tx: mpsc::UnboundedSender<PendingRequest>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    connecting = endpoint.accept() => {
                        if let Some(connecting) = connecting {
                            let connections = Arc::clone(&connections);
                            let request_tx = request_tx.clone();

                            tokio::spawn(async move {
                                match connecting.await {
                                    Ok(connection) => {
                                        let peer_id = match connection.remote_node_id() {
                                            Ok(id) => id,
                                            Err(e) => {
                                                warn!("Failed to get remote node ID: {e}");
                                                return;
                                            }
                                        };
                                        info!("Accepted connection from peer: {peer_id}");

                                        // Store connection
                                        {
                                            let mut conns = connections.write().await;
                                            conns.insert(peer_id, connection.clone());
                                        }

                                        // Start stream handler for this connection
                                        Self::handle_connection_streams(connection, peer_id, request_tx).await;
                                    }
                                    Err(e) => {
                                        warn!("Failed to accept connection: {e}");
                                    }
                                }
                            });
                        } else {
                            debug!("No more incoming connections");
                            break;
                        }
                    }
                    command = command_rx.recv() => {
                        match command {
                            Some(P2PCommand::Shutdown) => {
                                info!("Received shutdown command");
                                break;
                            }
                            None => {
                                debug!("Command channel closed");
                                break;
                            }
                        }
                    }
                }
            }
            info!("Connection handler stopped");
        })
    }

    /// Handle incoming streams on a connection
    async fn handle_connection_streams(
        connection: iroh::endpoint::Connection,
        peer_id: NodeId,
        request_tx: mpsc::UnboundedSender<PendingRequest>,
    ) {
        info!("Starting stream handler for peer: {peer_id}");

        loop {
            match connection.accept_bi().await {
                Ok((send_stream, recv_stream)) => {
                    info!("Accepted new stream from peer: {peer_id}");

                    let request_tx = request_tx.clone();
                    // Spawn handler for this stream
                    tokio::spawn(async move {
                        let mut stream = P2PStream::new(send_stream, recv_stream);
                        if let Err(e) = Self::handle_stream(&mut stream, peer_id, request_tx).await
                        {
                            warn!("Error handling stream from {peer_id}: {e}");
                        }
                    });
                }
                Err(e) => {
                    warn!("Failed to accept stream from {peer_id}: {e}");
                    break;
                }
            }
        }
    }

    /// Handle a single stream (inference request/response)
    #[allow(clippy::cognitive_complexity)]
    async fn handle_stream(
        stream: &mut P2PStream,
        peer_id: NodeId,
        request_tx: mpsc::UnboundedSender<PendingRequest>,
    ) -> Result<()> {
        info!("Handling stream from peer: {peer_id}");

        // Receive request
        let request = stream.recv_json().await?;
        info!("Received request from {peer_id}: {}", request);

        // Create response channel
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        // Send request to daemon for processing
        let pending_request = PendingRequest {
            payload: request,
            peer_id,
            response_tx,
        };

        if let Err(e) = request_tx.send(pending_request) {
            warn!("Failed to send request to daemon: {e}");
            return Err(P2PError::ConnectionFailed(
                "Request channel closed".to_string(),
            ));
        }

        // Wait for response from daemon
        match response_rx.await {
            Ok(response) => {
                stream.send_json(&response).await?;
                info!("Sent response to peer: {peer_id}");
            }
            Err(e) => {
                warn!("Failed to receive response from daemon: {e}");
                return Err(P2PError::ConnectionFailed(
                    "Response channel closed".to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl Default for P2PSessionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl P2PSession {
    /// Create a new builder
    #[must_use]
    pub const fn builder() -> P2PSessionBuilder {
        P2PSessionBuilder::new()
    }

    /// Get this node's ID
    #[must_use]
    pub fn node_id(&self) -> GateId {
        node_id_to_gate_id(self.node_id)
    }

    /// Get this node's address
    ///
    /// # Errors
    ///
    /// Returns an error if the node address cannot be retrieved
    pub async fn node_addr(&self) -> Result<GateAddr> {
        let node_addr = self.endpoint.node_addr().await?;
        Ok(node_addr_to_gate_addr(&node_addr))
    }

    /// Connect to a peer by address string
    ///
    /// # Errors
    ///
    /// Returns an error if address parsing or connection fails
    #[allow(clippy::cognitive_complexity)]
    pub async fn connect_str(&self, addr_str: &str) -> Result<GateId> {
        info!("Connecting to peer: {addr_str}");

        // Parse the address
        let node_addr = parse_node_addr(addr_str)?;
        let peer_id = node_addr.node_id;

        // Check if already connected
        {
            let connections = self.connections.read().await;
            if connections.contains_key(&peer_id) {
                info!("Already connected to peer: {peer_id}");
                return Ok(node_id_to_gate_id(peer_id));
            }
        }

        // Attempt to connect
        info!("Attempting to connect to: {peer_id}");
        let connection = self
            .endpoint
            .connect(node_addr, GATE_PROTOCOL)
            .await
            .map_err(|e| {
                P2PError::ConnectionFailed(format!("Failed to connect to {peer_id}: {e}"))
            })?;

        // Store the connection
        {
            let mut connections = self.connections.write().await;
            connections.insert(peer_id, connection);
        }

        info!("Successfully connected to peer: {peer_id}");
        Ok(node_id_to_gate_id(peer_id))
    }

    /// List connected peers
    pub async fn list_peers(&self) -> Vec<GateId> {
        let connections = self.connections.read().await;
        connections
            .keys()
            .map(|&node_id| node_id_to_gate_id(node_id))
            .collect()
    }

    /// Send a command to the P2P session
    ///
    /// # Errors
    ///
    /// Returns an error if the command channel is closed
    pub fn send_command(&self, command: P2PCommand) -> Result<()> {
        self.command_tx
            .send(command)
            .map_err(|_| P2PError::ConnectionFailed("Command channel closed".to_string()))?;
        Ok(())
    }

    /// Send inference request and receive response
    ///
    /// # Errors
    ///
    /// Returns an error if peer is not connected or communication fails
    pub async fn send_inference(&self, gate_id: GateId, request: JsonValue) -> Result<JsonValue> {
        info!("Sending inference request to peer {gate_id}");

        let peer_id = gate_id_to_node_id(gate_id);
        let mut stream = self.create_stream(peer_id).await?;
        stream.send_json(&request).await?;
        let response = stream.recv_json().await?;

        // Check for error response
        if let Some(error) = response.get("error") {
            let error_msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            return Err(P2PError::Protocol(format!(
                "Peer returned error: {error_msg}"
            )));
        }

        debug!("Received inference response from peer {gate_id}");
        Ok(response)
    }

    /// Start SNI proxy forwarding TLS traffic to peer
    ///
    /// # Errors
    ///
    /// Returns an error if peer is not connected or forwarding fails
    pub async fn start_sni_proxy<S>(&self, gate_id: GateId, other_stream: S) -> Result<(u64, u64)>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        info!("Starting SNI proxy to peer {gate_id}");

        let peer_id = gate_id_to_node_id(gate_id);
        let mut stream = self.create_stream(peer_id).await?;
        let (p2p_send, p2p_recv) = stream.streams();

        // Split the other stream
        let (mut other_read, mut other_write) = tokio::io::split(other_stream);

        // Forward in both directions concurrently
        let (result1, result2) = tokio::try_join!(
            tokio::io::copy(&mut other_read, p2p_send),
            tokio::io::copy(p2p_recv, &mut other_write)
        )
        .map_err(|e| P2PError::ConnectionFailed(format!("SNI proxy forwarding failed: {e}")))?;

        info!(
            "SNI proxy completed: {} bytes transferred",
            result1 + result2
        );
        Ok((result1, result2))
    }

    /// Perform handshake with peer
    ///
    /// # Errors
    ///
    /// Returns an error if handshake fails
    pub async fn handshake(&self, gate_id: GateId, capabilities: JsonValue) -> Result<JsonValue> {
        info!("Performing handshake with peer {gate_id}");

        let peer_id = gate_id_to_node_id(gate_id);
        let mut stream = self.create_stream(peer_id).await?;

        let handshake_request = serde_json::json!({
            "type": "handshake",
            "capabilities": capabilities
        });

        stream.send_json(&handshake_request).await?;
        let response = stream.recv_json().await?;

        let accepted = response
            .get("accepted")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if !accepted {
            let reason = response
                .get("reason")
                .and_then(|r| r.as_str())
                .unwrap_or("No reason provided");
            return Err(P2PError::Protocol(format!("Handshake rejected: {reason}")));
        }

        debug!("Handshake completed with peer {gate_id}");
        Ok(response)
    }

    /// Create a new stream to a peer
    async fn create_stream(&self, peer_id: NodeId) -> Result<P2PStream> {
        let connection = {
            let connections = self.connections.read().await;
            connections
                .get(&peer_id)
                .ok_or_else(|| P2PError::PeerNotFound(peer_id.to_string()))?
                .clone()
        };

        let (send_stream, recv_stream) = connection
            .open_bi()
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to open stream: {e}")))?;

        debug!("Created new stream to peer {peer_id}");
        Ok(P2PStream::new(send_stream, recv_stream))
    }

    /// Gracefully shutdown the session
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown fails
    pub async fn shutdown(&mut self) -> Result<()> {
        self.shutdown_with_timeout(Duration::from_secs(10)).await
    }

    /// Shutdown with custom timeout
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown fails
    pub async fn shutdown_with_timeout(&mut self, _timeout: Duration) -> Result<()> {
        info!("Starting P2P session shutdown");

        // Send shutdown command
        let _ = self.command_tx.send(P2PCommand::Shutdown);

        // Close all connections
        {
            let connections = self.connections.read().await;
            for (peer_id, connection) in connections.iter() {
                debug!("Closing connection to peer: {peer_id}");
                connection.close(0u8.into(), b"session shutdown");
            }
        }

        // Close endpoint
        self.endpoint.close().await;

        info!("P2P session shutdown completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_session_creation() {
        let (session, _request_rx) = P2PSession::builder().start().await.unwrap();
        let node_id = session.node_id();

        // Basic sanity check
        assert_ne!(node_id.to_string(), "");
    }

    #[test_log::test(tokio::test)]
    async fn test_empty_peer_list() {
        let (session, _request_rx) = P2PSession::builder().start().await.unwrap();
        let peers = session.list_peers().await;
        assert_eq!(peers.len(), 0);
    }

    #[test_log::test(tokio::test)]
    async fn test_builder_pattern() {
        let (_session, _request_rx) = P2PSession::builder().start().await.unwrap();
        // Builder should work without any configuration
    }
}
