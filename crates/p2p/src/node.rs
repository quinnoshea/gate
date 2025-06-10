//! P2P node implementation using Iroh

use iroh::{
    endpoint::{Connection, Endpoint},
    NodeAddr, NodeId,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::{P2PError, Result};

/// Gate protocol identifier
const GATE_PROTOCOL: &[u8] = b"gate/1.0";

/// P2P node for Gate network
pub struct P2PNode {
    endpoint: Endpoint,
    node_id: NodeId,
    connections: Arc<RwLock<HashMap<NodeId, Connection>>>,
    shutdown_token: CancellationToken,
    connection_handler_task: Option<JoinHandle<()>>,
}

impl P2PNode {
    /// Create a new P2P node
    ///
    /// # Errors
    ///
    /// Returns an error if the endpoint fails to bind or initialize.
    pub async fn new() -> Result<Self> {
        let endpoint = Endpoint::builder()
            .alpns(vec![GATE_PROTOCOL.to_vec()])
            .bind()
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to bind endpoint: {e}")))?;

        let node_id = endpoint.node_id();
        let connections = Arc::new(RwLock::new(HashMap::new()));
        let shutdown_token = CancellationToken::new();

        // Start connection handler automatically
        let connection_handler_task = {
            let connections_clone = Arc::clone(&connections);
            let endpoint_clone = endpoint.clone();
            let shutdown_token_clone = shutdown_token.clone();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        connecting = endpoint_clone.accept() => {
                            if let Some(connecting) = connecting {
                                let connections = Arc::clone(&connections_clone);
                                let shutdown_token = shutdown_token_clone.clone();

                                tokio::spawn(async move {
                                    match connecting.await {
                                        Ok(connection) => {
                                            let peer_id = match connection.remote_node_id() {
                                                Ok(id) => id,
                                                Err(e) => {
                                                    error!("Failed to get remote node ID: {e}");
                                                    return;
                                                }
                                            };
                                            info!("Accepted connection from peer: {peer_id}");

                                            // Store the connection
                                            {
                                                let mut conns = connections.write().await;
                                                conns.insert(peer_id, connection.clone());
                                            }

                                            // Handle the connection
                                            if let Err(e) = handle_connection(connection, shutdown_token).await {
                                                error!("Error handling connection from {peer_id}: {e}");
                                            }

                                            // Remove connection when done
                                            connections.write().await.remove(&peer_id);
                                            debug!("Removed connection for peer: {peer_id}");
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
                        () = shutdown_token_clone.cancelled() => {
                            info!("Connection handler shutting down");
                            break;
                        }
                    }
                }
            })
        };

        info!("P2P node created with ID: {node_id}");

        Ok(Self {
            endpoint,
            node_id,
            connections,
            shutdown_token,
            connection_handler_task: Some(connection_handler_task),
        })
    }

    /// Get this node's ID
    #[must_use]
    pub const fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Get this node's address that others can use to connect
    ///
    /// # Errors
    ///
    /// Returns an error if the node address cannot be retrieved.
    pub async fn node_addr(&self) -> Result<NodeAddr> {
        Ok(self.endpoint.node_addr().await?)
    }

    /// Connect to a peer
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails or cannot be established.
    pub async fn connect_to_peer(&self, addr: NodeAddr) -> Result<()> {
        let peer_id = addr.node_id;

        debug!("Attempting to connect to peer: {peer_id}");

        let connection = self
            .endpoint
            .connect(addr, GATE_PROTOCOL)
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
        Ok(())
    }

    /// Get list of connected peers
    pub async fn connected_peers(&self) -> Vec<NodeId> {
        let connections = self.connections.read().await;
        connections.keys().copied().collect()
    }

    /// Get connection to a specific peer
    async fn get_connection(&self, peer_id: NodeId) -> Option<Connection> {
        let connections = self.connections.read().await;
        connections.get(&peer_id).cloned()
    }

    /// Send a message to a peer (stub for now)
    ///
    /// # Errors
    ///
    /// Returns an error if the peer is not found or the message cannot be sent.
    pub async fn send_message(&self, peer_id: NodeId, message: &[u8]) -> Result<()> {
        let connection = self
            .get_connection(peer_id)
            .await
            .ok_or_else(|| P2PError::PeerNotFound(peer_id.to_string()))?;

        let (mut send_stream, _recv_stream) = connection
            .open_bi()
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to open stream: {e}")))?;

        send_stream
            .write_all(message)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to write: {e}")))?;

        send_stream
            .finish()
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to finish stream: {e}")))?;

        debug!("Sent message to peer {peer_id}");
        Ok(())
    }

    /// Gracefully shutdown the P2P node
    ///
    /// This will:
    /// 1. Signal all background tasks to stop
    /// 2. Close all active connections
    /// 3. Close the endpoint
    /// 4. Wait for tasks to complete (with timeout)
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown operations fail or timeout.
    pub async fn shutdown(&mut self) -> Result<()> {
        self.shutdown_with_timeout(Duration::from_secs(10)).await
    }

    /// Gracefully shutdown the P2P node with a custom timeout
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown operations fail or timeout.
    pub async fn shutdown_with_timeout(&mut self, timeout: Duration) -> Result<()> {
        info!("Starting graceful shutdown of P2P node");

        // Signal shutdown to all background tasks
        self.shutdown_token.cancel();

        // Close all connections
        self.close_all_connections().await;

        // Wait for connection handler task to complete
        self.wait_for_connection_handler(timeout).await;

        // Close the endpoint and clear connections
        self.finalize_shutdown().await;

        info!("P2P node shutdown completed");
        Ok(())
    }

    /// Close all active connections
    async fn close_all_connections(&self) {
        let connections = self.connections.read().await;
        for (peer_id, connection) in connections.iter() {
            debug!("Closing connection to peer: {peer_id}");
            connection.close(0u8.into(), b"node shutdown");
        }
    }

    /// Wait for connection handler task to complete with timeout
    async fn wait_for_connection_handler(&mut self, timeout: Duration) {
        if let Some(task) = self.connection_handler_task.take() {
            match tokio::time::timeout(timeout, task).await {
                Ok(Ok(())) => debug!("Connection handler task completed successfully"),
                Ok(Err(e)) => warn!("Connection handler task failed: {e}"),
                Err(_) => warn!("Connection handler task timed out during shutdown"),
            }
        }
    }

    /// Finalize shutdown by closing endpoint and clearing connections
    async fn finalize_shutdown(&self) {
        debug!("Closing endpoint");
        self.endpoint.close().await;

        let mut connections = self.connections.write().await;
        connections.clear();
    }

    /// Check if the node is shutting down
    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_token.is_cancelled()
    }
}

/// Handle an incoming connection (placeholder)
async fn handle_connection(
    connection: Connection,
    shutdown_token: CancellationToken,
) -> Result<()> {
    let peer_id = get_peer_id(&connection)?;
    debug!("Handling connection from {peer_id}");

    // Accept incoming streams and handle them
    connection_loop(connection, peer_id, shutdown_token).await;

    debug!("Connection handler for {peer_id} finished");
    Ok(())
}

/// Extract peer ID from connection
fn get_peer_id(connection: &Connection) -> Result<iroh::NodeId> {
    connection
        .remote_node_id()
        .map_err(|e| P2PError::ConnectionFailed(format!("Failed to get remote node ID: {e}")))
}

/// Main connection handling loop
async fn connection_loop(
    connection: Connection,
    peer_id: iroh::NodeId,
    shutdown_token: CancellationToken,
) {
    loop {
        let loop_result = handle_connection_event(&connection, peer_id, &shutdown_token).await;
        if let ConnectionEvent::Break(reason) = loop_result {
            debug!("Connection loop for {peer_id} ending: {reason}");
            break;
        }
    }
}

/// Represents the result of handling a connection event
enum ConnectionEvent {
    Continue,
    Break(String),
}

/// Handle a single connection event
async fn handle_connection_event(
    connection: &Connection,
    peer_id: iroh::NodeId,
    shutdown_token: &CancellationToken,
) -> ConnectionEvent {
    tokio::select! {
        stream = connection.accept_bi() => {
            handle_incoming_stream(stream, peer_id)
        }
        () = shutdown_token.cancelled() => {
            ConnectionEvent::Break("shutdown requested".to_string())
        }
        _ = connection.closed() => {
            ConnectionEvent::Break("connection closed".to_string())
        }
    }
}

/// Handle an incoming stream
fn handle_incoming_stream(
    stream_result: std::result::Result<
        (iroh::endpoint::SendStream, iroh::endpoint::RecvStream),
        iroh::endpoint::ConnectionError,
    >,
    peer_id: iroh::NodeId,
) -> ConnectionEvent {
    match handle_stream_result(stream_result, peer_id) {
        Ok(()) => ConnectionEvent::Continue,
        Err(e) => {
            error!("Failed to handle stream: {e}");
            ConnectionEvent::Break("stream handling failed".to_string())
        }
    }
}

/// Handle the result of accepting a bidirectional stream
fn handle_stream_result(
    stream_result: std::result::Result<
        (iroh::endpoint::SendStream, iroh::endpoint::RecvStream),
        iroh::endpoint::ConnectionError,
    >,
    peer_id: iroh::NodeId,
) -> Result<()> {
    match stream_result {
        Ok((_send_stream, mut recv_stream)) => {
            tokio::spawn(async move {
                handle_stream_data(&mut recv_stream, peer_id).await;
            });
            Ok(())
        }
        Err(e) => {
            error!("Failed to accept stream: {e}");
            Err(P2PError::ConnectionFailed(format!(
                "Stream accept failed: {e}"
            )))
        }
    }
}

/// Handle data from a stream
async fn handle_stream_data(recv_stream: &mut iroh::endpoint::RecvStream, peer_id: iroh::NodeId) {
    match recv_stream.read_to_end(1024 * 1024).await {
        Ok(buffer) => {
            debug!("Received {} bytes from {peer_id}", buffer.len());
            // TODO: Parse and handle the message
        }
        Err(e) => {
            error!("Failed to read from stream: {e}");
        }
    }
}
