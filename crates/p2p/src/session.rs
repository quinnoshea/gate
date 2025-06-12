//! P2P session with actor-based peer management

use crate::protocols::{CONTROL_PROTOCOL, INFERENCE_PROTOCOL, SNI_PROXY_PROTOCOL};
use crate::request::InferenceRequest;
use crate::{P2PError, P2PStream, Result};
use dashmap::DashMap;
use hellas_gate_core::{GateAddr, GateId};
use iroh::{
    endpoint::{Connection, Endpoint},
    protocol::{ProtocolHandler, Router},
    NodeAddr, NodeId,
};
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Handle for accessing inference request stream
pub struct InferenceHandle {
    rx: mpsc::UnboundedReceiver<InferenceRequest>,
}

impl InferenceHandle {
    /// Get the next inference request
    pub async fn next(&mut self) -> Option<InferenceRequest> {
        self.rx.recv().await
    }
}

/// Handle for managing SNI proxy streams and routing
pub struct SniProxyHandle {
    idle_streams: Arc<DashMap<GateId, Vec<P2PStream>>>,
}

impl SniProxyHandle {
    /// Create a new SNI proxy handle
    pub fn new() -> Self {
        Self {
            idle_streams: Arc::new(DashMap::new()),
        }
    }

    /// Add an idle SNI stream for a specific node
    pub fn add_idle_stream(&self, node_id: GateId, stream: P2PStream) {
        self.idle_streams
            .entry(node_id)
            .or_insert_with(Vec::new)
            .push(stream);

        debug!(
            "Added idle SNI stream for node {}, total: {}",
            hex::encode(node_id.as_bytes()),
            self.idle_streams
                .get(&node_id)
                .map(|v| v.len())
                .unwrap_or(0)
        );
    }

    /// Get an idle SNI stream for a specific node
    pub fn get_stream_for_node(&self, node_id: &GateId) -> Option<P2PStream> {
        if let Some(mut streams) = self.idle_streams.get_mut(node_id) {
            let stream = streams.pop();
            if let Some(_) = &stream {
                debug!(
                    "Retrieved idle SNI stream for node {}, remaining: {}",
                    hex::encode(node_id.as_bytes()),
                    streams.len()
                );
            }
            stream
        } else {
            None
        }
    }

    /// Get the number of idle SNI streams for a node
    pub fn get_idle_stream_count(&self, node_id: &GateId) -> usize {
        self.idle_streams
            .get(node_id)
            .map(|streams| streams.len())
            .unwrap_or(0)
    }
}

/// Protocol handler for inference requests
#[derive(Clone, Debug)]
struct InferenceProtocol {
    tx: mpsc::UnboundedSender<InferenceRequest>,
}

impl ProtocolHandler for InferenceProtocol {
    fn accept(
        &self,
        connection: Connection,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>> {
        let tx = self.tx.clone();
        Box::pin(async move {
            let peer_node_id = connection.remote_node_id()?;
            debug!("Accepted inference connection from peer: {peer_node_id}");

            // Accept the first stream
            let (send_stream, recv_stream) = connection.accept_bi().await?;
            let mut stream = P2PStream::new(send_stream, recv_stream);

            // Receive the inference request
            let request_data = stream
                .recv_json()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to receive request: {e}"))?;
            let request = InferenceRequest::new(peer_node_id, request_data, stream);

            if let Err(e) = tx.send(request) {
                warn!("Failed to send inference request: {e}");
            }

            Ok(())
        })
    }
}

/// Protocol handler for SNI proxy requests
#[derive(Clone, Debug)]
struct SniProxyProtocol {
    idle_streams: Arc<DashMap<GateId, Vec<P2PStream>>>,
}

impl ProtocolHandler for SniProxyProtocol {
    fn accept(
        &self,
        connection: Connection,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>> {
        let idle_streams = self.idle_streams.clone();
        Box::pin(async move {
            let peer_node_id = connection.remote_node_id()?;
            info!("RELAY: SniProxyProtocol::accept() called from peer: {peer_node_id}");

            // Accept the first stream - this will be stored as an idle stream
            info!("RELAY: Accepting bi-directional stream from daemon {peer_node_id}");
            info!(
                "RELAY: Connection has remote_node_id: {:?}",
                connection.remote_node_id()
            );

            let accept_result =
                tokio::time::timeout(std::time::Duration::from_secs(2), connection.accept_bi())
                    .await;

            let (send_stream, recv_stream) = match accept_result {
                Ok(Ok(streams)) => {
                    info!("RELAY: Successfully accepted bi-directional stream from {peer_node_id}");
                    streams
                }
                Ok(Err(e)) => {
                    warn!("RELAY: Error accepting bi-directional stream from {peer_node_id}: {e}");
                    return Err(anyhow::anyhow!("Failed to accept stream: {e}"));
                }
                Err(_) => {
                    warn!(
                        "RELAY: Timeout (2s) accepting bi-directional stream from {peer_node_id}"
                    );
                    return Err(anyhow::anyhow!("Timeout accepting stream"));
                }
            };

            let mut stream = P2PStream::new(send_stream, recv_stream);

            // Wait for handshake data from daemon
            info!("RELAY: Waiting for handshake data from daemon {peer_node_id}");
            let handshake_data =
                match tokio::time::timeout(std::time::Duration::from_secs(2), stream.recv_bytes())
                    .await
                {
                    Ok(Ok(data)) => {
                        info!(
                            "RELAY: Received handshake data from daemon {peer_node_id}: {:?}",
                            String::from_utf8_lossy(&data)
                        );
                        data
                    }
                    Ok(Err(e)) => {
                        warn!("RELAY: Error receiving handshake from daemon {peer_node_id}: {e}");
                        return Err(anyhow::anyhow!("Failed to receive handshake: {e}"));
                    }
                    Err(_) => {
                        warn!("RELAY: Timeout waiting for handshake from daemon {peer_node_id}");
                        return Err(anyhow::anyhow!("Timeout waiting for handshake"));
                    }
                };

            if handshake_data != b"READY" {
                warn!(
                    "RELAY: Invalid handshake from daemon {peer_node_id}: {:?}",
                    String::from_utf8_lossy(&handshake_data)
                );
                return Err(anyhow::anyhow!("Invalid handshake data"));
            }

            // Store this as an idle stream for the peer
            let gate_id = hellas_gate_core::GateId::from_bytes(*peer_node_id.as_bytes());
            idle_streams
                .entry(gate_id)
                .or_insert_with(Vec::new)
                .push(stream);

            info!(
                "RELAY: Stored idle SNI stream for peer: {peer_node_id}, total: {}",
                idle_streams.get(&gate_id).map(|v| v.len()).unwrap_or(0)
            );

            // Return immediately - don't wait forever
            // The stream is now stored and ready to be used by the relay
            info!("RELAY: SniProxyProtocol::accept() returning for peer: {peer_node_id}");
            Ok(())
        })
    }
}

/// Control protocol handler (always enabled)
#[derive(Clone)]
struct ControlProtocol {
    capabilities: Vec<String>,
    dns_challenge_handler: Option<Arc<dyn DnsChallengeHandler>>,
    relay_peer_handles: Arc<RwLock<HashMap<GateId, JoinHandle<Result<()>>>>>,
}

impl std::fmt::Debug for ControlProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlProtocol")
            .field("capabilities", &self.capabilities)
            .field(
                "dns_challenge_handler",
                &self.dns_challenge_handler.is_some(),
            )
            .field("relay_peer_handles", &"<HashMap>")
            .finish()
    }
}

impl ProtocolHandler for ControlProtocol {
    fn accept(
        &self,
        connection: Connection,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>> {
        let capabilities = self.capabilities.clone();
        let dns_challenge_handler = self.dns_challenge_handler.clone();
        let relay_peer_handles = self.relay_peer_handles.clone();

        Box::pin(async move {
            let peer_node_id = connection.remote_node_id()?;
            let peer_id = GateId::from_bytes(*peer_node_id.as_bytes());

            info!(
                "RELAY: ControlProtocol accepting connection from peer: {}",
                peer_node_id
            );

            // Accept incoming control connection
            let (send_stream, recv_stream) = connection.accept_bi().await?;
            let control_stream = P2PStream::new(send_stream, recv_stream);

            // Create RelayPeerActor to handle this incoming connection
            let relay_peer_actor =
                RelayPeerActor::new(peer_id, control_stream, capabilities, dns_challenge_handler)
                    .await;

            // Spawn the RelayPeerActor task
            let actor_handle = tokio::spawn(async move { relay_peer_actor.run().await });

            // Store the handle for cleanup
            {
                let mut handles = relay_peer_handles.write().await;
                handles.insert(peer_id, actor_handle);
            }

            info!(
                "RELAY: ControlProtocol created RelayPeerActor for peer: {}",
                peer_node_id
            );

            // The RelayPeerActor will handle all communication from here
            // This function returns immediately since the actor is now running
            Ok(())
        })
    }
}

/// Control protocol messages sent between peers
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ControlMessage {
    /// Handshake message to establish connection
    Handshake {
        node_id: String,
        capabilities: Vec<String>,
    },
    /// Response to handshake
    HandshakeResponse {
        accepted: bool,
        capabilities: Vec<String>,
    },
    /// Ping for keep-alive
    Ping { id: u64 },
    /// Pong response
    Pong { id: u64 },
    /// Request a new stream for SNI proxy
    RequestSniStream { request_id: String, domain: String },
    /// Request a new stream for inference
    RequestInferenceStream { request_id: String },
    /// Response indicating stream is ready
    StreamReady {
        request_id: String,
        stream_accepted: bool,
        error: Option<String>,
    },
    /// Request DNS challenge creation for ACME
    DnsChallengeCreate {
        request_id: String,
        domain: String,
        txt_value: String,
    },
    /// Request DNS challenge cleanup for ACME
    DnsChallengeCleanup { request_id: String, domain: String },
    /// Response to DNS challenge request
    DnsChallengeResponse {
        request_id: String,
        success: bool,
        error: Option<String>,
    },
    /// Request domain registration from relay
    DomainRegistrationRequest {
        request_id: String,
        node_addr: GateAddr,
    },
    /// Response to domain registration request
    DomainRegistrationResponse {
        request_id: String,
        success: bool,
        domain: Option<String>,
        error: Option<String>,
    },
}

/// Handle for tracking peer connection status
pub struct PeerConnectionHandle {
    pub peer_id: GateId,
    connection_ready: tokio::sync::oneshot::Receiver<Result<()>>,
}

impl PeerConnectionHandle {
    /// Wait for the peer connection to be established
    pub async fn wait_connected(self) -> Result<GateId> {
        match self.connection_ready.await {
            Ok(Ok(())) => {
                info!("Peer {} connection established", self.peer_id);
                Ok(self.peer_id)
            }
            Ok(Err(e)) => {
                warn!("Peer {} connection failed: {}", self.peer_id, e);
                Err(e)
            }
            Err(_) => {
                warn!("Peer {} connection channel closed", self.peer_id);
                Err(P2PError::ConnectionFailed(
                    "Connection channel closed".to_string(),
                ))
            }
        }
    }

    /// Get the peer ID without waiting
    pub fn peer_id(&self) -> GateId {
        self.peer_id
    }
}

/// Actor that manages a single peer connection (client-side, daemon connects to relay)
struct PeerActor {
    peer_id: GateId,
    peer_addr: GateAddr,
    endpoint: Endpoint,
    control_stream: Option<P2PStream>,
    command_rx: mpsc::UnboundedReceiver<PeerCommand>,
    connection_ready_tx: Option<tokio::sync::oneshot::Sender<Result<()>>>,
    capabilities: Vec<String>,
    dns_challenge_handler: Option<Arc<dyn DnsChallengeHandler>>,
    /// Track pending DNS challenge requests
    pending_dns_requests:
        std::collections::HashMap<String, tokio::sync::oneshot::Sender<Result<()>>>,
}

/// Actor that manages a single incoming peer connection (server-side, relay accepts from daemon)
struct RelayPeerActor {
    peer_id: GateId,
    control_stream: P2PStream,
    capabilities: Vec<String>,
    dns_challenge_handler: Option<Arc<dyn DnsChallengeHandler>>,
}

/// Commands sent to peer actors
#[derive(Debug)]
enum PeerCommand {
    /// Open an inference stream
    OpenInference {
        response_tx: tokio::sync::oneshot::Sender<Result<P2PStream>>,
    },
    /// Send DNS challenge create request
    DnsChallengeCreate {
        request_id: String,
        domain: String,
        txt_value: String,
        response_tx: tokio::sync::oneshot::Sender<Result<()>>,
    },
    /// Send DNS challenge cleanup request
    DnsChallengeCleanup {
        request_id: String,
        domain: String,
        response_tx: tokio::sync::oneshot::Sender<Result<()>>,
    },
    /// Shutdown the peer actor
    Shutdown,
}

impl PeerActor {
    async fn new(
        peer_addr: GateAddr,
        endpoint: Endpoint,
        connection_ready_tx: Option<tokio::sync::oneshot::Sender<Result<()>>>,
        capabilities: Vec<String>,
        dns_challenge_handler: Option<Arc<dyn DnsChallengeHandler>>,
    ) -> Result<(Self, mpsc::UnboundedSender<PeerCommand>)> {
        let peer_id = peer_addr.id;
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        let actor = Self {
            peer_id,
            peer_addr,
            endpoint,
            control_stream: None,
            command_rx,
            connection_ready_tx,
            capabilities,
            dns_challenge_handler,
            pending_dns_requests: std::collections::HashMap::new(),
        };

        Ok((actor, command_tx))
    }

    async fn run(mut self) -> Result<()> {
        // Establish control channel
        let connection_result = self.establish_control_channel().await;

        // Signal connection status
        if let Some(tx) = self.connection_ready_tx.take() {
            let _ = tx.send(
                connection_result
                    .as_ref()
                    .map(|_| ())
                    .map_err(|e| P2PError::ConnectionFailed(format!("{}", e))),
            );
        }

        // If connection failed, exit early
        connection_result?;

        // Main actor loop
        loop {
            tokio::select! {
                // Handle commands from session
                Some(command) = self.command_rx.recv() => {
                    match command {
                        PeerCommand::OpenInference { response_tx } => {
                            let result = self.request_inference_stream().await;
                            let _ = response_tx.send(result);
                        }
                        PeerCommand::DnsChallengeCreate { request_id, domain, txt_value, response_tx } => {
                            info!("DAEMON: PeerActor handling DNS challenge create command for {} - request_id: {}, domain: {}",
                                  self.peer_id, request_id, domain);

                            // Store the response channel for when we get the response
                            self.pending_dns_requests.insert(request_id.clone(), response_tx);
                            info!("DAEMON: PeerActor stored pending DNS request: {} (total pending: {})",
                                  request_id, self.pending_dns_requests.len());

                            // Send the request
                            if let Err(e) = self.send_dns_challenge_create(request_id.clone(), domain, txt_value).await {
                                warn!("DAEMON: PeerActor failed to send DNS challenge create request: {}", e);
                                // Remove from pending and send error immediately if send failed
                                if let Some(tx) = self.pending_dns_requests.remove(&request_id) {
                                    let _ = tx.send(Err(e));
                                }
                            } else {
                                info!("DAEMON: PeerActor successfully sent DNS challenge create request: {}", request_id);
                            }
                        }
                        PeerCommand::DnsChallengeCleanup { request_id, domain, response_tx } => {
                            // Store the response channel for when we get the response
                            self.pending_dns_requests.insert(request_id.clone(), response_tx);

                            // Send the request
                            if let Err(e) = self.send_dns_challenge_cleanup(request_id.clone(), domain).await {
                                // Remove from pending and send error immediately if send failed
                                if let Some(tx) = self.pending_dns_requests.remove(&request_id) {
                                    let _ = tx.send(Err(e));
                                }
                            }
                        }
                        PeerCommand::Shutdown => {
                            info!("Shutting down peer actor for {}", self.peer_id);
                            break;
                        }
                    }
                }

                // Handle incoming control messages immediately without timeout
                _ = async {
                    if let Some(ref mut stream) = self.control_stream {
                        match stream.recv_json().await {
                            Ok(msg_json) => {
                                match serde_json::from_value::<ControlMessage>(msg_json) {
                                    Ok(msg) => {
                                        info!("DAEMON: PeerActor received control message from {}: {:?}", self.peer_id, msg);
                                        self.handle_control_message(msg).await;
                                    }
                                    Err(e) => {
                                        warn!("DAEMON: Failed to parse control message from {}: {}", self.peer_id, e);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("DAEMON: Control stream error for peer {}: {}", self.peer_id, e);
                                // Don't break here, let the main loop handle shutdown
                            }
                        }
                    }
                } => {}
            }
        }

        Ok(())
    }

    async fn establish_control_channel(&mut self) -> Result<()> {
        let node_id = NodeId::from_bytes(self.peer_id.as_bytes())
            .map_err(|e| P2PError::Protocol(format!("Invalid node ID: {e}")))?;

        // Parse address from first direct address
        if self.peer_addr.direct_addresses.is_empty() {
            return Err(P2PError::Protocol(
                "No direct addresses available for peer".to_string(),
            ));
        }

        let socket_addr = self.peer_addr.direct_addresses[0];
        let node_addr = NodeAddr::from_parts(node_id, None, vec![socket_addr]);

        // Connect and establish control stream
        let connection = self
            .endpoint
            .connect(node_addr, CONTROL_PROTOCOL)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to connect: {e}")))?;

        let (send_stream, recv_stream) = connection.open_bi().await.map_err(|e| {
            P2PError::ConnectionFailed(format!("Failed to open control stream: {e}"))
        })?;

        self.control_stream = Some(P2PStream::new(send_stream, recv_stream));

        // Send handshake
        let handshake = ControlMessage::Handshake {
            node_id: hex::encode(self.peer_id.as_bytes()),
            capabilities: self.capabilities.clone(),
        };

        if let Some(ref mut stream) = self.control_stream {
            let msg_json = serde_json::to_value(handshake)
                .map_err(|e| P2PError::Protocol(format!("Failed to serialize handshake: {e}")))?;
            stream.send_json(&msg_json).await?;
        }

        info!("Established control channel with peer {}", self.peer_id);

        // Wait for handshake response with timeout
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.receive_control_message(),
        )
        .await
        .map_err(|_| P2PError::Protocol("Handshake timeout".to_string()))?
        .map_err(|e| P2PError::Protocol(format!("Failed to receive handshake response: {e}")))?;

        match response {
            ControlMessage::HandshakeResponse {
                accepted,
                capabilities,
            } => {
                if accepted {
                    info!("Handshake accepted by peer {}", self.peer_id);

                    // Note peer capabilities for potential use by external handlers
                    if capabilities.contains(&"sni_proxy".to_string()) {
                        info!("Peer {} supports SNI proxy", self.peer_id);
                        // External applications can handle SNI stream management
                    }
                } else {
                    return Err(P2PError::Protocol("Handshake rejected by peer".to_string()));
                }
            }
            _ => {
                return Err(P2PError::Protocol(
                    "Expected handshake response".to_string(),
                ));
            }
        }

        Ok(())
    }

    async fn receive_control_message(&mut self) -> Result<ControlMessage> {
        if let Some(ref mut stream) = self.control_stream {
            let msg_json = stream.recv_json().await?;
            let msg = serde_json::from_value(msg_json)
                .map_err(|e| P2PError::Protocol(format!("Failed to parse control message: {e}")))?;
            Ok(msg)
        } else {
            Err(P2PError::Protocol("No control stream".to_string()))
        }
    }

    async fn handle_control_message(&mut self, msg: ControlMessage) {
        match msg {
            ControlMessage::Handshake {
                node_id: _,
                capabilities,
            } => {
                // Respond to handshake with our capabilities
                let response = ControlMessage::HandshakeResponse {
                    accepted: true,
                    capabilities: self.capabilities.clone(),
                };

                if let Some(ref mut stream) = self.control_stream {
                    if let Ok(response_json) = serde_json::to_value(response) {
                        let _ = stream.send_json(&response_json).await;
                    }
                }

                // Note peer capabilities for potential use by external handlers
                if capabilities.contains(&"sni_proxy".to_string()) {
                    info!("Peer {} supports SNI proxy", self.peer_id);
                    // External applications can handle SNI stream management
                }
            }
            ControlMessage::HandshakeResponse { .. } => {
                // HandshakeResponse is now handled in establish_control_channel during connection setup
                debug!(
                    "Received unexpected handshake response from peer {}",
                    self.peer_id
                );
            }
            ControlMessage::Ping { id } => {
                let pong = ControlMessage::Pong { id };
                if let Some(ref mut stream) = self.control_stream {
                    if let Ok(pong_json) = serde_json::to_value(pong) {
                        let _ = stream.send_json(&pong_json).await;
                    }
                }
            }
            ControlMessage::RequestSniStream {
                request_id: _,
                domain,
            } => {
                // TODO: Handle incoming SNI stream requests
                debug!("Received SNI stream request for domain: {}", domain);
            }
            ControlMessage::RequestInferenceStream { request_id: _ } => {
                // TODO: Handle incoming inference stream requests
                debug!("Received inference stream request");
            }
            ControlMessage::DnsChallengeCreate {
                request_id,
                domain,
                txt_value,
            } => {
                debug!(
                    "Received DNS challenge create request for domain: {}, txt_value: {}",
                    domain, txt_value
                );

                let (success, error) = if let Some(ref handler) = self.dns_challenge_handler {
                    match handler
                        .handle_dns_challenge_create(&domain, &txt_value)
                        .await
                    {
                        Ok(_record_id) => (true, None),
                        Err(err) => (false, Some(err)),
                    }
                } else {
                    debug!("No DNS challenge handler available");
                    (
                        false,
                        Some("No DNS challenge handler available".to_string()),
                    )
                };

                // Send response
                let response = ControlMessage::DnsChallengeResponse {
                    request_id,
                    success,
                    error,
                };

                if let Some(ref mut stream) = self.control_stream {
                    if let Ok(response_json) = serde_json::to_value(response) {
                        let _ = stream.send_json(&response_json).await;
                    }
                }
            }
            ControlMessage::DnsChallengeCleanup { request_id, domain } => {
                debug!(
                    "Received DNS challenge cleanup request for domain: {}",
                    domain
                );

                let (success, error) = if let Some(ref handler) = self.dns_challenge_handler {
                    match handler.handle_dns_challenge_cleanup(&domain).await {
                        Ok(()) => (true, None),
                        Err(err) => (false, Some(err)),
                    }
                } else {
                    debug!("No DNS challenge handler available");
                    (
                        false,
                        Some("No DNS challenge handler available".to_string()),
                    )
                };

                // Send response
                let response = ControlMessage::DnsChallengeResponse {
                    request_id,
                    success,
                    error,
                };

                if let Some(ref mut stream) = self.control_stream {
                    if let Ok(response_json) = serde_json::to_value(response) {
                        let _ = stream.send_json(&response_json).await;
                    }
                }
            }
            ControlMessage::DnsChallengeResponse {
                request_id,
                success,
                error,
            } => {
                info!("DAEMON: Received DNS challenge response: request_id={}, success={}, error={:?}", request_id, success, error);

                // Complete the pending request if it exists
                if let Some(response_tx) = self.pending_dns_requests.remove(&request_id) {
                    let result = if success {
                        Ok(())
                    } else {
                        Err(P2PError::Protocol(
                            error.unwrap_or_else(|| "DNS challenge failed".to_string()),
                        ))
                    };

                    info!("DAEMON: Completing pending DNS challenge request: request_id={}, result={:?}", request_id, result);
                    if let Err(_) = response_tx.send(result) {
                        warn!("DAEMON: Failed to send DNS challenge response - receiver dropped for request_id: {}", request_id);
                    } else {
                        info!(
                            "DAEMON: Successfully completed DNS challenge request: request_id={}",
                            request_id
                        );
                    }
                } else {
                    warn!(
                        "DAEMON: Received DNS challenge response for unknown request_id: {}",
                        request_id
                    );
                }
            }
            _ => {
                debug!("Received control message: {:?}", msg);
            }
        }
    }

    // SNI stream management has been moved to external applications
    // The P2P crate now only provides basic peer connection and control protocol functionality

    async fn request_inference_stream(&mut self) -> Result<P2PStream> {
        // TODO: Send control message to request inference stream
        // For now, just open a direct stream
        let node_id = NodeId::from_bytes(self.peer_id.as_bytes())
            .map_err(|e| P2PError::Protocol(format!("Invalid node ID: {e}")))?;

        // Use first direct address
        if self.peer_addr.direct_addresses.is_empty() {
            return Err(P2PError::Protocol(
                "No direct addresses available for peer".to_string(),
            ));
        }

        let socket_addr = self.peer_addr.direct_addresses[0];
        let node_addr = NodeAddr::from_parts(node_id, None, vec![socket_addr]);

        let connection = self
            .endpoint
            .connect(node_addr, INFERENCE_PROTOCOL)
            .await
            .map_err(|e| {
                P2PError::ConnectionFailed(format!("Failed to connect for inference: {e}"))
            })?;

        let (send_stream, recv_stream) = connection.open_bi().await.map_err(|e| {
            P2PError::ConnectionFailed(format!("Failed to open inference stream: {e}"))
        })?;

        Ok(P2PStream::new(send_stream, recv_stream))
    }

    async fn send_dns_challenge_create(
        &mut self,
        request_id: String,
        domain: String,
        txt_value: String,
    ) -> Result<()> {
        info!("DAEMON: PeerActor sending DNS challenge create request to {} - request_id: {}, domain: {}",
              self.peer_id, request_id, domain);

        let request = ControlMessage::DnsChallengeCreate {
            request_id: request_id.clone(),
            domain: domain.clone(),
            txt_value,
        };

        if let Some(ref mut stream) = self.control_stream {
            let msg_json = serde_json::to_value(request).map_err(|e| {
                P2PError::Protocol(format!(
                    "Failed to serialize DNS challenge create request: {e}"
                ))
            })?;

            info!(
                "DAEMON: PeerActor sending JSON to {}: {:?}",
                self.peer_id, msg_json
            );
            stream.send_json(&msg_json).await?;
            info!(
                "DAEMON: PeerActor sent DNS challenge create request to peer {} - request_id: {}",
                self.peer_id, request_id
            );
            Ok(())
        } else {
            Err(P2PError::Protocol(
                "No control stream available".to_string(),
            ))
        }
    }

    async fn send_dns_challenge_cleanup(
        &mut self,
        request_id: String,
        domain: String,
    ) -> Result<()> {
        let request = ControlMessage::DnsChallengeCleanup { request_id, domain };

        if let Some(ref mut stream) = self.control_stream {
            let msg_json = serde_json::to_value(request).map_err(|e| {
                P2PError::Protocol(format!(
                    "Failed to serialize DNS challenge cleanup request: {e}"
                ))
            })?;
            stream.send_json(&msg_json).await?;
            debug!(
                "Sent DNS challenge cleanup request to peer {}",
                self.peer_id
            );
            Ok(())
        } else {
            Err(P2PError::Protocol(
                "No control stream available".to_string(),
            ))
        }
    }
}

impl RelayPeerActor {
    /// Create a new relay peer actor for an incoming connection
    async fn new(
        peer_id: GateId,
        control_stream: P2PStream,
        capabilities: Vec<String>,
        dns_challenge_handler: Option<Arc<dyn DnsChallengeHandler>>,
    ) -> Self {
        Self {
            peer_id,
            control_stream,
            capabilities,
            dns_challenge_handler,
        }
    }

    /// Run the relay peer actor to handle incoming messages
    async fn run(mut self) -> Result<()> {
        info!("RELAY: RelayPeerActor started for peer {}", self.peer_id);

        // Main message handling loop
        loop {
            match self.control_stream.recv_json().await {
                Ok(msg_json) => {
                    debug!(
                        "RELAY: RelayPeerActor received message from {}: {:?}",
                        self.peer_id, msg_json
                    );

                    // Parse and handle control messages
                    match serde_json::from_value::<ControlMessage>(msg_json) {
                        Ok(msg) => {
                            info!(
                                "RELAY: RelayPeerActor handling message from {}: {:?}",
                                self.peer_id, msg
                            );
                            self.handle_control_message(msg).await;
                        }
                        Err(e) => {
                            warn!(
                                "RELAY: RelayPeerActor failed to parse message from {}: {}",
                                self.peer_id, e
                            );
                        }
                    }
                }
                Err(e) => {
                    // Connection closed or error
                    info!(
                        "RELAY: RelayPeerActor connection from {} closed: {}",
                        self.peer_id, e
                    );
                    break;
                }
            }
        }

        info!("RELAY: RelayPeerActor finished for peer {}", self.peer_id);
        Ok(())
    }

    /// Handle incoming control messages from daemon
    async fn handle_control_message(&mut self, msg: ControlMessage) {
        match msg {
            ControlMessage::Handshake {
                node_id: _,
                capabilities,
            } => {
                // Respond to handshake with our capabilities
                let response = ControlMessage::HandshakeResponse {
                    accepted: true,
                    capabilities: self.capabilities.clone(),
                };

                if let Ok(response_json) = serde_json::to_value(response) {
                    if let Err(e) = self.control_stream.send_json(&response_json).await {
                        warn!(
                            "RELAY: Failed to send handshake response to {}: {}",
                            self.peer_id, e
                        );
                    }
                }

                // Log peer capabilities
                if capabilities.contains(&"inference".to_string()) {
                    info!("RELAY: Peer {} supports inference", self.peer_id);
                }
            }
            ControlMessage::DnsChallengeCreate {
                request_id,
                domain,
                txt_value,
            } => {
                info!("RELAY: RelayPeerActor received DNS challenge create request from {} - request_id: {}, domain: {}, txt_value: {}",
                      self.peer_id, request_id, domain, txt_value);

                let (success, error) = if let Some(ref handler) = self.dns_challenge_handler {
                    info!(
                        "RELAY: RelayPeerActor calling DNS challenge handler for domain: {}",
                        domain
                    );
                    match handler
                        .handle_dns_challenge_create(&domain, &txt_value)
                        .await
                    {
                        Ok(_record_id) => {
                            info!("RELAY: RelayPeerActor DNS challenge created successfully for domain: {}", domain);
                            (true, None)
                        }
                        Err(err) => {
                            warn!("RELAY: RelayPeerActor failed to create DNS challenge for domain {}: {}", domain, err);
                            (false, Some(err))
                        }
                    }
                } else {
                    warn!("RELAY: RelayPeerActor no DNS challenge handler available for request_id: {}", request_id);
                    (
                        false,
                        Some("No DNS challenge handler available".to_string()),
                    )
                };

                // Send response back to daemon
                let response = ControlMessage::DnsChallengeResponse {
                    request_id: request_id.clone(),
                    success,
                    error: error.clone(),
                };

                info!("RELAY: RelayPeerActor sending DNS challenge response to {} - request_id: {}, success: {}, error: {:?}",
                      self.peer_id, request_id, success, error);

                if let Ok(response_json) = serde_json::to_value(response) {
                    if let Err(e) = self.control_stream.send_json(&response_json).await {
                        warn!(
                            "RELAY: RelayPeerActor failed to send DNS challenge response to {}: {}",
                            self.peer_id, e
                        );
                    } else {
                        info!("RELAY: RelayPeerActor DNS challenge response sent successfully to {} for request_id: {}", self.peer_id, request_id);
                    }
                } else {
                    warn!("RELAY: RelayPeerActor failed to serialize DNS challenge response for request_id: {}", request_id);
                }
            }
            ControlMessage::DnsChallengeCleanup { request_id, domain } => {
                info!("RELAY: RelayPeerActor received DNS challenge cleanup request from {} - request_id: {}, domain: {}",
                      self.peer_id, request_id, domain);

                let (success, error) = if let Some(ref handler) = self.dns_challenge_handler {
                    info!("RELAY: RelayPeerActor calling DNS challenge cleanup handler for domain: {}", domain);
                    match handler.handle_dns_challenge_cleanup(&domain).await {
                        Ok(()) => {
                            info!("RELAY: RelayPeerActor DNS challenge cleaned up successfully for domain: {}", domain);
                            (true, None)
                        }
                        Err(err) => {
                            warn!("RELAY: RelayPeerActor failed to cleanup DNS challenge for domain {}: {}", domain, err);
                            (false, Some(err))
                        }
                    }
                } else {
                    warn!("RELAY: RelayPeerActor no DNS challenge handler available for cleanup request_id: {}", request_id);
                    (
                        false,
                        Some("No DNS challenge handler available".to_string()),
                    )
                };

                // Send response back to daemon
                let response = ControlMessage::DnsChallengeResponse {
                    request_id: request_id.clone(),
                    success,
                    error: error.clone(),
                };

                info!("RELAY: RelayPeerActor sending DNS challenge cleanup response to {} - request_id: {}, success: {}, error: {:?}",
                      self.peer_id, request_id, success, error);

                if let Ok(response_json) = serde_json::to_value(response) {
                    if let Err(e) = self.control_stream.send_json(&response_json).await {
                        warn!("RELAY: RelayPeerActor failed to send DNS challenge cleanup response to {}: {}", self.peer_id, e);
                    } else {
                        info!("RELAY: RelayPeerActor DNS challenge cleanup response sent successfully to {} for request_id: {}", self.peer_id, request_id);
                    }
                } else {
                    warn!("RELAY: RelayPeerActor failed to serialize DNS challenge cleanup response for request_id: {}", request_id);
                }
            }
            ControlMessage::Ping { id } => {
                debug!(
                    "RELAY: RelayPeerActor received ping from {} with id: {}",
                    self.peer_id, id
                );
                let pong = ControlMessage::Pong { id };
                if let Ok(pong_json) = serde_json::to_value(pong) {
                    if let Err(e) = self.control_stream.send_json(&pong_json).await {
                        warn!(
                            "RELAY: RelayPeerActor failed to send pong response to {}: {}",
                            self.peer_id, e
                        );
                    }
                }
            }
            _ => {
                debug!(
                    "RELAY: RelayPeerActor received unsupported control message from {}: {:?}",
                    self.peer_id, msg
                );
            }
        }
    }
}

/// P2P session that manages multiple peer connections
pub struct P2PSession {
    endpoint: Endpoint,
    node_id: NodeId,
    _router: Router,
    peers: Arc<RwLock<HashMap<GateId, mpsc::UnboundedSender<PeerCommand>>>>,
    peer_handles: Arc<RwLock<HashMap<GateId, JoinHandle<Result<()>>>>>,
    relay_peer_handles: Arc<RwLock<HashMap<GateId, JoinHandle<Result<()>>>>>,
    inference_handle: Option<InferenceHandle>,
    sni_proxy_handle: Option<SniProxyHandle>,
    capabilities: Vec<String>,
    dns_challenge_handler: Option<Arc<dyn DnsChallengeHandler>>,
}

pub struct P2PSessionBuilder {
    identity: Option<iroh::SecretKey>,
    port: u16,
    inference_enabled: bool,
    sni_proxy_enabled: bool,
    dns_challenge_enabled: bool,
    dns_challenge_handler: Option<Arc<dyn DnsChallengeHandler>>,
}

/// Trait for handling DNS challenge requests (implemented by relay)
pub trait DnsChallengeHandler: Send + Sync {
    /// Handle DNS challenge create request
    fn handle_dns_challenge_create(
        &self,
        domain: &str,
        txt_value: &str,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>> + Send + '_>>;

    /// Handle DNS challenge cleanup request
    fn handle_dns_challenge_cleanup(
        &self,
        domain: &str,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), String>> + Send + '_>>;
}

impl P2PSessionBuilder {
    pub const fn new() -> Self {
        Self {
            identity: None,
            port: 0,
            inference_enabled: false,
            sni_proxy_enabled: false,
            dns_challenge_enabled: false,
            dns_challenge_handler: None,
        }
    }

    pub fn with_identity(mut self, identity: iroh::SecretKey) -> Self {
        self.identity = Some(identity);
        self
    }

    pub const fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Enable inference protocol support (daemon nodes)
    pub const fn with_inference(mut self) -> Self {
        self.inference_enabled = true;
        self
    }

    /// Enable SNI proxy protocol support (relay nodes)
    pub const fn with_sni_proxy(mut self) -> Self {
        self.sni_proxy_enabled = true;
        self
    }

    /// Enable DNS challenge support (relay nodes)
    pub const fn with_dns_challenge(mut self) -> Self {
        self.dns_challenge_enabled = true;
        self
    }

    /// Set DNS challenge handler (relay nodes)
    pub fn with_dns_challenge_handler(mut self, handler: Arc<dyn DnsChallengeHandler>) -> Self {
        self.dns_challenge_handler = Some(handler);
        self
    }

    pub fn with_generated_identity(mut self) -> Self {
        let mut rng = rand::thread_rng();
        self.identity = Some(iroh::SecretKey::generate(&mut rng));
        self
    }

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

    pub async fn build(self) -> Result<P2PSession> {
        // Build list of ALPNs based on enabled protocols
        let mut alpns = vec![CONTROL_PROTOCOL.to_vec()];
        if self.inference_enabled {
            alpns.push(INFERENCE_PROTOCOL.to_vec());
        }
        if self.sni_proxy_enabled {
            alpns.push(SNI_PROXY_PROTOCOL.to_vec());
        }

        let mut endpoint_builder = Endpoint::builder()
            .alpns(alpns)
            .relay_mode(iroh::RelayMode::Disabled);

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

        // Build capabilities list based on enabled protocols
        let mut capabilities = Vec::new();
        if self.inference_enabled {
            capabilities.push("inference".to_string());
        }
        if self.sni_proxy_enabled {
            capabilities.push("sni_proxy".to_string());
        }
        if self.dns_challenge_enabled {
            capabilities.push("dns_challenge".to_string());
        }

        info!("P2P Session capabilities: {:?}", capabilities);

        // Create relay peer handles for incoming connections (used by relay nodes)
        let relay_peer_handles = Arc::new(RwLock::new(HashMap::new()));

        // Create router with protocol handlers
        let mut router_builder = Router::builder(endpoint.clone());

        // Always register control protocol
        info!(
            "Registering CONTROL_PROTOCOL: {:?}",
            std::str::from_utf8(CONTROL_PROTOCOL)
        );
        router_builder = router_builder.accept(
            CONTROL_PROTOCOL,
            ControlProtocol {
                capabilities: capabilities.clone(),
                dns_challenge_handler: self.dns_challenge_handler.clone(),
                relay_peer_handles: relay_peer_handles.clone(),
            },
        );

        // Set up optional protocol channels and handlers
        let inference_handle = if self.inference_enabled {
            let (tx, rx) = mpsc::unbounded_channel();
            router_builder = router_builder.accept(INFERENCE_PROTOCOL, InferenceProtocol { tx });
            Some(InferenceHandle { rx })
        } else {
            None
        };

        let sni_proxy_handle = if self.sni_proxy_enabled {
            info!(
                "Registering SNI_PROXY_PROTOCOL: {:?}",
                std::str::from_utf8(SNI_PROXY_PROTOCOL)
            );
            let handle = SniProxyHandle::new();
            router_builder = router_builder.accept(
                SNI_PROXY_PROTOCOL,
                SniProxyProtocol {
                    idle_streams: handle.idle_streams.clone(),
                },
            );
            Some(handle)
        } else {
            info!("SNI_PROXY_PROTOCOL not enabled");
            None
        };

        let router = router_builder.spawn();

        let node_id = endpoint.node_id();
        info!("P2P session started with ID: {node_id}");

        Ok(P2PSession {
            endpoint,
            node_id,
            _router: router,
            peers: Arc::new(RwLock::new(HashMap::new())),
            peer_handles: Arc::new(RwLock::new(HashMap::new())),
            relay_peer_handles,
            inference_handle,
            sni_proxy_handle,
            capabilities,
            dns_challenge_handler: self.dns_challenge_handler,
        })
    }
}

impl Default for P2PSessionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl P2PSession {
    pub const fn builder() -> P2PSessionBuilder {
        P2PSessionBuilder::new()
    }

    pub fn node_id(&self) -> GateId {
        GateId::from_bytes(*self.node_id.as_bytes())
    }

    pub async fn node_addr(&self) -> Result<GateAddr> {
        let node_addr = self.endpoint.node_addr().await?;
        let direct_addrs: Vec<std::net::SocketAddr> =
            node_addr.direct_addresses().copied().collect();

        Ok(GateAddr::new(self.node_id(), direct_addrs))
    }

    /// Add a peer and establish persistent connection
    /// Returns a handle that can be awaited to check connection success
    pub async fn add_peer(&self, peer_addr: GateAddr) -> Result<PeerConnectionHandle> {
        let peer_id = peer_addr.id;

        // Create a channel to signal when connection is established
        let (connection_tx, connection_rx) = tokio::sync::oneshot::channel();

        // Create peer actor
        let (actor, command_tx) = PeerActor::new(
            peer_addr.clone(),
            self.endpoint.clone(),
            Some(connection_tx),
            self.capabilities.clone(),
            self.dns_challenge_handler.clone(),
        )
        .await?;

        // Spawn peer actor
        let handle = tokio::spawn(async move { actor.run().await });

        // Store peer command channel and handle
        {
            let mut peers = self.peers.write().await;
            let mut handles = self.peer_handles.write().await;
            peers.insert(peer_id, command_tx);
            handles.insert(peer_id, handle);
        }

        info!("Adding peer {} ({})", peer_id, peer_addr);
        Ok(PeerConnectionHandle {
            peer_id,
            connection_ready: connection_rx,
        })
    }

    /// Get handle for receiving SNI proxy requests (for relay nodes)
    pub fn take_sni_proxy_handle(&mut self) -> Option<SniProxyHandle> {
        self.sni_proxy_handle.take()
    }

    /// Get handle for receiving inference requests (for daemon nodes)
    pub fn take_inference_handle(&mut self) -> Option<InferenceHandle> {
        self.inference_handle.take()
    }

    /// List connected peers
    pub async fn list_peers(&self) -> Vec<GateId> {
        let peers = self.peers.read().await;
        peers.keys().copied().collect()
    }

    /// Send DNS challenge create request to a peer (relay)
    ///
    /// # Errors
    ///
    /// Returns an error if the peer is not connected or the request fails
    pub async fn request_dns_challenge_create(
        &self,
        peer_id: GateId,
        domain: String,
        txt_value: String,
    ) -> Result<String> {
        let request_id = uuid::Uuid::new_v4().to_string();

        info!(
            "Requesting DNS challenge creation for {} on peer {}",
            domain, peer_id
        );

        // Send command to the peer actor to handle DNS challenge creation
        let peers = self.peers.read().await;
        if let Some(peer_tx) = peers.get(&peer_id) {
            let (response_tx, response_rx) = tokio::sync::oneshot::channel();

            let command = PeerCommand::DnsChallengeCreate {
                request_id: request_id.clone(),
                domain,
                txt_value,
                response_tx,
            };

            peer_tx.send(command).map_err(|_| {
                P2PError::PeerNotFound(format!("Failed to send command to peer {}", peer_id))
            })?;

            // Wait for response
            response_rx
                .await
                .map_err(|_| {
                    P2PError::Protocol("Failed to receive response from peer".to_string())
                })?
                .map_err(|e| P2PError::Protocol(format!("DNS challenge request failed: {e}")))?;

            info!(
                "DNS challenge create request sent successfully: {}",
                request_id
            );
        } else {
            return Err(P2PError::PeerNotFound(format!(
                "Peer {} not found",
                peer_id
            )));
        }

        Ok(request_id)
    }

    /// Send DNS challenge cleanup request to a peer (relay)
    ///
    /// # Errors
    ///
    /// Returns an error if the peer is not connected or the request fails
    pub async fn request_dns_challenge_cleanup(
        &self,
        peer_id: GateId,
        domain: String,
    ) -> Result<String> {
        let request_id = uuid::Uuid::new_v4().to_string();

        info!(
            "Requesting DNS challenge cleanup for {} on peer {}",
            domain, peer_id
        );

        // Send command to the peer actor to handle DNS challenge cleanup
        let peers = self.peers.read().await;
        if let Some(peer_tx) = peers.get(&peer_id) {
            let (response_tx, response_rx) = tokio::sync::oneshot::channel();

            let command = PeerCommand::DnsChallengeCleanup {
                request_id: request_id.clone(),
                domain,
                response_tx,
            };

            peer_tx.send(command).map_err(|_| {
                P2PError::PeerNotFound(format!("Failed to send command to peer {}", peer_id))
            })?;

            // Wait for response
            response_rx
                .await
                .map_err(|_| {
                    P2PError::Protocol("Failed to receive response from peer".to_string())
                })?
                .map_err(|e| P2PError::Protocol(format!("DNS challenge cleanup failed: {e}")))?;

            info!(
                "DNS challenge cleanup request sent successfully: {}",
                request_id
            );
        } else {
            return Err(P2PError::PeerNotFound(format!(
                "Peer {} not found",
                peer_id
            )));
        }

        Ok(request_id)
    }

    /// Request domain registration from a relay peer
    ///
    /// # Errors
    ///
    /// Returns an error if the peer is not connected or the request fails
    pub async fn request_domain_registration(
        &self,
        peer_id: GateId,
        node_addr: GateAddr,
    ) -> Result<String> {
        info!(
            "Requesting domain registration from relay peer: {}",
            peer_id
        );

        // Send domain registration request through control channel
        let request_id = uuid::Uuid::new_v4().to_string();
        let _request = ControlMessage::DomainRegistrationRequest {
            request_id: request_id.clone(),
            node_addr,
        };

        // TODO: Send request through peer control channel and wait for response
        // For now, simulate the request and return a mock domain
        let node_id_hex = hex::encode(peer_id.as_bytes());
        // Truncate to 32 characters (16 bytes) to match daemon certificate generation
        // and comply with DNS label length limit (63 chars max)
        let truncated_node_id = &node_id_hex[..32];
        let mock_domain = format!("{}.private.hellas.ai", truncated_node_id);

        info!("Mock domain registration response: {}", mock_domain);
        Ok(mock_domain)
    }
}
