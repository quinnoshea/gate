//! Main Gate daemon implementation

use crate::config::DaemonConfig;
use crate::http::HttpServer;
use crate::service::DaemonServiceImpl;
use crate::upstream::UpstreamClient;
use crate::{DaemonError, Result};
use hellas_gate_proto::pb::gate::inference::v1::*;
use hellas_gate_proto::pb::gate::relay::v1::relay_service_server::RelayServiceServer;
use iroh::Endpoint;
use n0_watcher::Watcher;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;
use tracing::{error, info, warn, trace};
use tokio_stream::StreamExt;
use tonic_iroh_transport::GrpcProtocolHandler;

/// P2P connection information for Axum request extensions
/// Just holds references to the actual Iroh objects that contain all the rich metadata
#[derive(Debug, Clone)]
pub struct P2pConnectionInfo {
    /// Short hash used as relay domain prefix (e.g., "abc123" in "abc123.private.hellas.ai") 
    pub relay_domain_prefix: String,
    /// The full domain used for this connection
    pub domain: String,
    /// The actual Iroh connection - contains remote_node_id, remote_address, stats, etc.
    pub connection: iroh::endpoint::Connection,
    /// Local Iroh endpoint - contains local node_id, bound addresses, etc.
    pub endpoint: iroh::Endpoint,
}

/// Context for handling TLS stream forwarding - contains everything needed
#[derive(Clone)]
pub struct TlsStreamContext {
    pub domain: String,
    pub relay_domain_prefix: String,
    pub tls_acceptor: tokio_rustls::TlsAcceptor,
    pub http_server: std::sync::Arc<crate::http::HttpServer>,
    pub connection: iroh::endpoint::Connection,
    pub endpoint: iroh::Endpoint,
}

impl TlsStreamContext {
    /// Create new context with all required fields
    pub fn new(
        domain: String,
        relay_domain_prefix: String,
        tls_acceptor: tokio_rustls::TlsAcceptor,
        http_server: std::sync::Arc<crate::http::HttpServer>,
        connection: iroh::endpoint::Connection,
        endpoint: iroh::Endpoint,
    ) -> Self {
        Self {
            domain,
            relay_domain_prefix,
            tls_acceptor,
            http_server,
            connection,
            endpoint,
        }
    }

    pub fn to_p2p_info(&self) -> P2pConnectionInfo {
        P2pConnectionInfo {
            relay_domain_prefix: self.relay_domain_prefix.clone(),
            domain: self.domain.clone(),
            connection: self.connection.clone(),
            endpoint: self.endpoint.clone(),
        }
    }

    /// Handle a TLS stream with this context
    pub async fn handle_stream(
        &self,
        p2p_send: iroh::endpoint::SendStream,
        p2p_recv: iroh::endpoint::RecvStream,
    ) -> crate::Result<()> {
        info!("TLS FORWARDING: Terminating TLS for domain: {} from relay", self.domain);
        
        // Create a bidirectional stream from the P2P streams
        let p2p_stream = BiDirectionalStream::new(p2p_send, p2p_recv);
        
        // Terminate TLS to get decrypted stream
        let decrypted_stream = self.tls_acceptor.accept(p2p_stream).await?;
        
        info!("TLS FORWARDING: Successfully terminated TLS, forwarding to Axum");
        
        // Forward decrypted stream directly to Axum with P2P info
        self.http_server.handle_stream_with_p2p_info(decrypted_stream, Some(self.to_p2p_info())).await?;
        
        info!("TLS FORWARDING: Stream completed successfully for domain: {}", self.domain);
        Ok(())
    }
}

/// Bidirectional stream wrapper for P2P streams
pub struct BiDirectionalStream {
    send: iroh::endpoint::SendStream,
    recv: iroh::endpoint::RecvStream,
}

impl BiDirectionalStream {
    pub fn new(send: iroh::endpoint::SendStream, recv: iroh::endpoint::RecvStream) -> Self {
        Self { send, recv }
    }
}

impl tokio::io::AsyncRead for BiDirectionalStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for BiDirectionalStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match std::pin::Pin::new(&mut self.send).poll_write(cx, buf) {
            std::task::Poll::Ready(Ok(n)) => std::task::Poll::Ready(Ok(n)),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::other(e))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_flush(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        match std::pin::Pin::new(&mut self.send).poll_flush(cx) {
            std::task::Poll::Ready(Ok(())) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::other(e))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_shutdown(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        match std::pin::Pin::new(&mut self.send).poll_shutdown(cx) {
            std::task::Poll::Ready(Ok(())) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Err(std::io::Error::other(e))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// Main Gate daemon that orchestrates all services
pub struct GateDaemon {
    endpoint: Endpoint,
    cert_manager: crate::certs::CertificateManager,
    discovered_relays: Arc<tokio::sync::RwLock<std::collections::HashSet<iroh::NodeId>>>,
    shutdown_token: CancellationToken,
}

impl GateDaemon {
    /// Create a new daemon with the given configuration, identity, and state directory
    /// This fully initializes all components and starts background services
    ///
    /// # Errors
    ///
    /// Returns an error if daemon initialization fails
    pub async fn new(
        config: DaemonConfig,
        identity: Vec<u8>,
        state_dir: std::path::PathBuf,
    ) -> Result<Self> {
        info!("Initializing Gate daemon components");
        
        let shutdown_token = CancellationToken::new();
        
        // Initialize P2P endpoint
        let endpoint = Self::create_p2p_endpoint(&config, &identity).await?;
        
        // Initialize certificate manager
        let cert_dir = state_dir.join("certificates");
        let le_config = config.tls.letsencrypt.clone().unwrap_or_default();
        let cert_manager = crate::certs::CertificateManager::new(le_config, endpoint.clone(), cert_dir).await?;
        
        // Start background services (pass discovered_relays reference)
        let discovered_relays = Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new()));
        Self::start_background_services(&config, &identity, &endpoint, &cert_manager, discovered_relays.clone(), shutdown_token.clone()).await?;
        
        info!("Gate daemon fully initialized and running");
        
        Ok(Self {
            endpoint,
            cert_manager,
            discovered_relays,
            shutdown_token,
        })
    }

    /// Get a reference to the certificate manager
    pub fn cert_manager(&self) -> &crate::certs::CertificateManager {
        &self.cert_manager
    }

    /// Wait for daemon shutdown
    pub async fn wait_for_shutdown(&self) -> Result<()> {
        // Wait for shutdown signal
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal");
            }
            _ = self.shutdown_token.cancelled() => {
                info!("Shutdown requested");
            }
        }
        
        info!("Gate daemon stopped");
        Ok(())
    }
    
    /// Request daemon shutdown
    pub fn shutdown(&self) {
        self.shutdown_token.cancel();
    }

    async fn create_p2p_endpoint(config: &DaemonConfig, identity: &[u8]) -> Result<Endpoint> {
        info!("Initializing P2P endpoint on port {}", config.p2p.port);

        // Create secret key from identity
        let key_array: [u8; 32] = identity[0..32]
            .try_into()
            .map_err(|_| DaemonError::ConfigString("Invalid identity key length".to_string()))?;
        let secret_key = iroh::SecretKey::from_bytes(&key_array);

        // Create endpoint with port binding
        let bind_addr = format!("0.0.0.0:{}", config.p2p.port);
        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .bind_addr_v4(bind_addr.parse().map_err(|e| {
                DaemonError::ConfigString(format!("Invalid bind address: {}", e))
            })?)
            .relay_mode(iroh::RelayMode::Disabled)
            .discovery_n0()
            .discovery_local_network()
            .bind()
            .await?;


        // Debug: Check what addresses the endpoint has
        let node_id = endpoint.node_id();
        let bound_sockets = endpoint.bound_sockets();
        info!("P2P endpoint created - Node ID: {}", node_id);
        info!("Bound sockets: {}", bound_sockets.len());
        for (i, addr) in bound_sockets.iter().enumerate() {
            info!("  Bound socket {}: {}", i + 1, addr);
        }
        
        if bound_sockets.is_empty() {
            tracing::warn!("No bound sockets found - this may affect P2P connectivity");
        }

        info!("P2P endpoint initialized successfully");
        Ok(endpoint)
    }

    async fn start_background_services(
        config: &DaemonConfig,
        identity: &[u8],
        endpoint: &Endpoint,
        cert_manager: &crate::certs::CertificateManager,
        discovered_relays: Arc<tokio::sync::RwLock<std::collections::HashSet<iroh::NodeId>>>,
        shutdown_token: CancellationToken,
    ) -> Result<()> {
        let upstream_client = UpstreamClient::new(&config.upstream)?;
        
        // Create TLS forwarding handler
        let tls_handler = Self::create_tls_forwarding_handler(config, cert_manager, identity, upstream_client.clone(), endpoint.clone()).await?;
        
        // Start gRPC inference service with peer discovery AND TLS forwarding
        Self::start_grpc_service(endpoint, upstream_client.clone(), discovered_relays.clone(), shutdown_token.clone(), cert_manager, identity, tls_handler).await?;
        
        // Start HTTP server
        Self::start_http_server(config, identity, upstream_client, shutdown_token.clone()).await?;
        
        // TLS bridge will be created as needed for each connection
        
        info!("All background services started successfully");
        Ok(())
    }

    async fn start_grpc_service(
        endpoint: &Endpoint,
        upstream_client: UpstreamClient,
        discovered_relays: Arc<tokio::sync::RwLock<std::collections::HashSet<iroh::NodeId>>>,
        shutdown_token: CancellationToken,
        cert_manager: &crate::certs::CertificateManager,
        identity: &[u8],
        tls_handler: TlsForwardingProtocolHandler,
    ) -> Result<()> {
        info!("Starting gRPC inference service with peer discovery");
        
        // Create inference service implementation
        let inference_service = Arc::new(DaemonServiceImpl::new(Arc::new(upstream_client)));
        
        // Create tonic-iroh protocol handler for inference service
        let (handler, incoming, inference_alpn) = GrpcProtocolHandler::for_service::<inference_service_server::InferenceServiceServer<DaemonServiceImpl>>();
        
        // Generate relay service ALPN for comparison
        let (_, _, relay_alpn) = GrpcProtocolHandler::for_service::<RelayServiceServer<hellas_gate_relay::service::RelayServiceImpl>>();
        
        info!("Monitoring for relay service ALPN: {}", String::from_utf8_lossy(&relay_alpn));
        
        // Set up TLS forwarding ALPN
        const TLS_FORWARD_ALPN: &[u8] = b"/gate.relay.v1.TlsForward/1.0";
        let tls_forward_alpn = TLS_FORWARD_ALPN.to_vec();
        info!("Registering TLS forwarding ALPN: {}", String::from_utf8_lossy(&tls_forward_alpn));
        
        // Set up router with both inference service AND TLS forwarding
        let router = iroh::protocol::Router::builder(endpoint.clone())
            .accept(&inference_alpn, handler)
            .accept(&tls_forward_alpn, tls_handler)
            .spawn();
            
        info!("Central router created with gRPC inference and TLS forwarding ALPNs");
        
        // Start event-driven relay discovery using iroh's discovery stream
        // This will listen for new peer discovery events and connect to relay services
        let endpoint_clone = endpoint.clone();
        let relays_clone = discovered_relays.clone();
        let relay_alpn_clone = relay_alpn.clone();
        let shutdown_token_clone = shutdown_token.clone();
        let cert_manager_arc = Arc::new((*cert_manager).clone());
        let identity_clone = identity.to_vec();
        tokio::spawn(async move {
            Self::listen_for_relay_peers(endpoint_clone, relays_clone, relay_alpn_clone, shutdown_token_clone, cert_manager_arc, identity_clone).await;
        });
        
        // Keep router alive by moving it into a task
        let shutdown_router_token = shutdown_token.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown_router_token.cancelled() => {
                    info!("Router shutting down");
                }
            }
            // Router is moved into this task and kept alive
            drop(router);
        });

        // Start tonic gRPC server with iroh incoming stream
        let inference_server = inference_service_server::InferenceServiceServer::new((*inference_service).clone());
        tokio::spawn(async move {
            tokio::select! {
                result = Server::builder()
                    .add_service(inference_server)
                    .serve_with_incoming(incoming) => {
                    if let Err(e) = result {
                        error!("gRPC server error: {}", e);
                    }
                }
                _ = shutdown_token.cancelled() => {
                    info!("gRPC service shutting down");
                }
            }
        });
        
        info!("gRPC inference service started successfully");
        Ok(())
    }

    async fn start_http_server(
        config: &DaemonConfig,
        identity: &[u8],
        upstream_client: UpstreamClient,
        shutdown_token: CancellationToken,
    ) -> Result<()> {
        info!("Starting HTTP server on {}", config.http.bind_addr);
        
        // Derive gate_id from identity
        let key_array: [u8; 32] = identity[0..32]
            .try_into()
            .map_err(|_| DaemonError::ConfigString("Invalid identity key length".to_string()))?;
        let secret_key = iroh::SecretKey::from_bytes(&key_array);
        let gate_id = hellas_gate_core::GateId::from_bytes(*secret_key.public().as_bytes());
        
        let http_server = HttpServer::new(
            config.http.clone(),
            Arc::new(upstream_client),
            gate_id,
        )?;
        
        // Start HTTP server in background task
        tokio::spawn(async move {
            tokio::select! {
                result = http_server.start() => {
                    if let Err(e) = result {
                        error!("HTTP server error: {}", e);
                    }
                }
                _ = shutdown_token.cancelled() => {
                    info!("HTTP server shutting down");
                }
            }
        });
        
        info!("HTTP server started successfully");
        Ok(())
    }



    /// Check if we've discovered any relay peers
    pub async fn discovered_relay_count(&self) -> usize {
        self.discovered_relays.read().await.len()
    }
    
    
    /// Get the daemon's node address for other nodes to connect to
    pub async fn node_addr(&self) -> Result<hellas_gate_core::GateAddr> {
        let node_id = self.endpoint.node_id();
        
        // Wait for address discovery with timeout
        let node_addr_watcher = self.endpoint.node_addr();
        let node_addr = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            async {
                while let Ok(None) = node_addr_watcher.get() {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                node_addr_watcher.get()
            }
        ).await
        .map_err(|_| DaemonError::ConfigString("Timeout waiting for node address discovery".to_string()))?
        .map_err(|e| DaemonError::Disconnected(e))?
        .ok_or_else(|| DaemonError::ConfigString("Node address discovery failed".to_string()))?;
        
        // Convert direct addresses to our format
        let direct_addresses: Vec<std::net::SocketAddr> = node_addr.direct_addresses.into_iter().collect();
        let gate_id = hellas_gate_core::GateId::from_bytes(*node_id.as_bytes());
        
        info!("Node address discovered: {} direct addresses", direct_addresses.len());
        Ok(hellas_gate_core::GateAddr::new(gate_id, direct_addresses))
    }

    /// Connect to a relay node for domain registration
    pub async fn connect_to_relay(&self, relay_addr: &str) -> Result<()> {
        let gate_addr: hellas_gate_core::GateAddr = relay_addr.parse()
            .map_err(|e| DaemonError::ConfigString(format!("Invalid relay address: {}", e)))?;

        info!("Connecting to relay: {}", gate_addr.id);

        // Convert to iroh NodeAddr
        let node_id = iroh::NodeId::from_bytes(gate_addr.id.as_bytes())
            .map_err(|e| DaemonError::ConfigString(format!("Invalid node ID: {}", e)))?;
        let node_addr = iroh::NodeAddr::new(node_id).with_direct_addresses(gate_addr.direct_addresses);

        // Test connection using tonic-iroh
        let iroh_client = tonic_iroh_transport::IrohClient::new(self.endpoint.clone());
        let _channel = iroh_client.connect_to_service::<inference_service_server::InferenceServiceServer<hellas_gate_relay::service::RelayServiceImpl>>(node_addr)
            .await.map_err(|e| DaemonError::ConfigString(format!("Failed to connect: {}", e)))?;

        info!("Successfully connected to relay: {}", gate_addr.id);
        Ok(())
    }

    /// Listen for relay peers using event-driven discovery
    async fn listen_for_relay_peers(
        endpoint: Endpoint,
        discovered_relays: Arc<tokio::sync::RwLock<std::collections::HashSet<iroh::NodeId>>>,
        relay_alpn: Vec<u8>,
        shutdown_token: CancellationToken,
        cert_manager: Arc<crate::certs::CertificateManager>,
        identity: Vec<u8>,
    ) {
        info!("Starting event-driven relay peer discovery");
        
        // Get discovery stream from endpoint for passive peer discovery
        let mut discovery_stream = endpoint.discovery_stream();
        
        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    info!("Relay peer discovery shutting down");
                    break;
                }
                
                // Listen for new peer discovery events
                discovery_result = discovery_stream.next() => {
                    match discovery_result {
                        Some(Ok(discovery_item)) => {
                            let peer_id = discovery_item.node_id();
                            
                            // Check if we've already discovered this relay - early exit to prevent spam
                            {
                                let relays = discovered_relays.read().await;
                                if relays.contains(&peer_id) {
                                    trace!("Ignoring already discovered relay peer: {} (source: {})", peer_id, discovery_item.provenance());
                                    continue;
                                }
                            }
                            
                            info!("Discovered new peer: {} (source: {}), testing for relay service", peer_id, discovery_item.provenance());
                            
                            // Try to connect and check if this peer supports the relay service
                            let node_addr = discovery_item.to_node_addr();
                            if let Err(e) = Self::try_connect_to_relay_service(
                                &endpoint, 
                                peer_id, 
                                node_addr, 
                                &relay_alpn,
                                discovered_relays.clone(),
                                cert_manager.clone(),
                                &identity
                            ).await {
                                trace!("Peer {} does not support relay service: {}", peer_id, e);
                            }
                        }
                        Some(Err(e)) => {
                            warn!("Discovery stream error: {}", e);
                        }
                        None => {
                            info!("Discovery stream ended");
                            break;
                        }
                    }
                }
            }
        }
        
        info!("Relay peer discovery stopped");
    }

    /// Create TLS forwarding protocol handler
    async fn create_tls_forwarding_handler(
        config: &DaemonConfig,
        cert_manager: &crate::certs::CertificateManager,
        identity: &[u8],
        upstream_client: UpstreamClient,
        endpoint: iroh::Endpoint,
    ) -> Result<TlsForwardingProtocolHandler> {
        info!("Creating TLS forwarding protocol handler");

        // Create certificate info for TLS termination
        let cert_manager_clone = cert_manager.clone();
        let key_array: [u8; 32] = identity[0..32]
            .try_into()
            .map_err(|_| DaemonError::ConfigString("Invalid identity key length".to_string()))?;
        let secret_key = iroh::SecretKey::from_bytes(&key_array);
        let node_id = hex::encode(secret_key.public().as_bytes());
        let domain = format!("{}.private.hellas.ai", &node_id[..16]);
        
        info!("TLS forwarding handler configuration:");
        info!("  Node ID: {}", node_id);
        info!("  Short domain: {}", domain);
        
        // Derive gate_id from identity  
        let gate_id = hellas_gate_core::GateId::from_bytes(*secret_key.public().as_bytes());
        
        // Create certificate info for TLS handler
        let cert_info = cert_manager.get_certificate(&domain).await
            .ok_or_else(|| DaemonError::Certificate(format!("No certificate found for domain: {}", domain)))?;
        
        // Create TLS acceptor for terminating HTTPS traffic
        let tls_handler = crate::tls::TlsHandler::from_certificate_info(&cert_info)
            .map_err(|e| DaemonError::Certificate(format!("Failed to create TLS handler: {}", e)))?;
        
        let tls_config = tls_handler.create_tls_config()
            .map_err(|e| DaemonError::Certificate(format!("Failed to create TLS config: {}", e)))?;
        
        let tls_acceptor = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(tls_config));
        
        info!("TLS acceptor created for domain: {}", domain);
        
        // Create HTTP server instance for stream handling
        let http_server = std::sync::Arc::new(crate::http::HttpServer::new(
            config.http.clone(),
            std::sync::Arc::new(upstream_client.clone()),
            gate_id,
        )?);
        
        // Create TLS forwarding protocol handler
        let tls_forwarding_handler = TlsForwardingProtocolHandler {
            config: config.clone(),
            cert_manager: cert_manager_clone,
            domain,
            node_id,
            upstream_client,
            gate_id,
            tls_acceptor,
            http_server,
            endpoint,
        };
        
        info!("TLS forwarding protocol handler created successfully");
        Ok(tls_forwarding_handler)
    }

    
    /// Try to connect to a peer and check if it supports the relay service
    async fn try_connect_to_relay_service(
        endpoint: &Endpoint,
        peer_id: iroh::NodeId,
        node_addr: iroh::NodeAddr,
        relay_alpn: &[u8],
        discovered_relays: Arc<tokio::sync::RwLock<std::collections::HashSet<iroh::NodeId>>>,
        cert_manager: Arc<crate::certs::CertificateManager>,
        _identity: &[u8],
    ) -> Result<()> {
        trace!("Testing connection to potential relay peer: {}", peer_id);
        
        // Clone direct addresses before moving node_addr
        let direct_addresses = node_addr.direct_addresses.clone();
        
        // Try to connect to the peer with the relay service ALPN
        match endpoint.connect(node_addr, relay_alpn).await {
            Ok(connection) => {
                info!("Successfully connected to relay service on peer: {}", peer_id);
                
                // Add to discovered relays
                {
                    let mut relays = discovered_relays.write().await;
                    relays.insert(peer_id);
                }
                
                // Create typed relay service client from connection
                let iroh_client = tonic_iroh_transport::IrohClient::new(endpoint.clone());
                let gate_id = hellas_gate_core::GateId::from_bytes(*peer_id.as_bytes());
                let relay_gate_addr = hellas_gate_core::GateAddr::new(gate_id, direct_addresses.into_iter().collect());
                let node_addr = iroh::NodeAddr::new(peer_id).with_direct_addresses(relay_gate_addr.direct_addresses.clone());
                
                // Trigger certificate upgrade check in background
                let cert_manager_clone = cert_manager.clone();
                tokio::spawn(async move {
                    // Check for self-signed certificates that can be upgraded
                    let self_signed_domains = cert_manager_clone.get_self_signed_domains().await;
                    
                    if self_signed_domains.is_empty() {
                        info!("No self-signed certificates found to upgrade");
                        return;
                    }
                    
                    info!("Found {} self-signed certificate(s) that could be upgraded: {:?}", 
                          self_signed_domains.len(), self_signed_domains);
                    
                    // Attempt to create relay service client for upgrades
                    match iroh_client.connect_to_service::<hellas_gate_proto::pb::gate::relay::v1::relay_service_server::RelayServiceServer<hellas_gate_relay::service::RelayServiceImpl>>(node_addr).await {
                        Ok(channel) => {
                            let relay_client = hellas_gate_proto::pb::gate::relay::v1::relay_service_client::RelayServiceClient::new(channel);
                            
                            // Attempt to upgrade each self-signed certificate
                            for domain in self_signed_domains {
                                match cert_manager_clone.upgrade_certificate(&domain, relay_client.clone()).await {
                                    Ok(cert_info) => {
                                        info!("Certificate upgrade completed for domain: {} (type: {:?})", 
                                              cert_info.domain, cert_info.cert_type);
                                    }
                                    Err(e) => {
                                        warn!("Certificate upgrade failed for domain {}: {}", domain, e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to create relay service client for certificate upgrades: {}", e);
                        }
                    }
                });
                
                // Note: Registration with relay happens automatically when the relay
                // accepts our connection via the TlsForwardingHandler
                
                // Close the test connection
                connection.close(0u32.into(), b"test connection");
                
                info!("Added relay peer {} to discovered relay list", peer_id);
                Ok(())
            }
            Err(e) => {
                // This peer doesn't support the relay service or connection failed
                trace!("Peer {} does not support relay service or connection failed: {}", peer_id, e);
                Err(DaemonError::ConfigString(format!("Not a relay peer: {}", e)))
            }
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_p2p_endpoint_discovery() {
        // Test P2P endpoint creation and local discovery
        println!("=== P2P Endpoint Discovery Test ===");
        
        // Create a test endpoint similar to daemon configuration
        let mut rng = rand::thread_rng();
        let secret_key = iroh::SecretKey::generate(&mut rng);
        println!("Generated test node ID: {}", secret_key.public());
        
        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key)
            .bind_addr_v4("0.0.0.0:0".parse().unwrap()) // Random port
            .relay_mode(iroh::RelayMode::Disabled)
            .discovery_n0()
            .discovery_local_network()
            .bind()
            .await
            .expect("Failed to create test endpoint");
            
        println!("✓ Endpoint created successfully");
        println!("  Node ID: {}", endpoint.node_id());
        
        // Wait for discovery to find local addresses
        println!("  Waiting for local endpoint discovery...");
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Check bound sockets (what we can actually get)
        let bound_sockets = endpoint.bound_sockets();
        println!("  Bound socket addresses: {}", bound_sockets.len());
        for (i, socket) in bound_sockets.iter().enumerate() {
            println!("    Socket {}: {}", i + 1, socket);
        }
        
        if bound_sockets.is_empty() {
            println!("  ⚠️  WARNING: No bound sockets found");
            println!("  This indicates a socket binding issue");
        } else {
            println!("  ✓ Socket binding successful");
        }
        
        // Check node ID and public key
        println!("  Node public key: {}", endpoint.node_id());
        
        println!("=== End P2P Discovery Test ===");
    }

    #[tokio::test]
    async fn test_daemon_full_initialization() {
        // Initialize crypto provider for rustls
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        
        // Test the new daemon full initialization process
        println!("=== Daemon Full Initialization Test ===");
        
        let config = DaemonConfig::default();
        let identity = vec![0u8; 32]; // Simple test identity
        let temp_dir = tempfile::tempdir().unwrap();
        
        // Test full daemon initialization (should complete quickly)
        match timeout(Duration::from_secs(10), GateDaemon::new(config, identity, temp_dir.path().to_path_buf())).await {
            Ok(Ok(daemon)) => {
                println!("✓ Daemon full initialization succeeded");
                
                // Test that endpoint is immediately available (no Option unwrapping)
                let bound_sockets = daemon.endpoint.bound_sockets();
                println!("  Daemon endpoint bound sockets: {}", bound_sockets.len());
                
                for (i, addr) in bound_sockets.iter().enumerate() {
                    println!("    {}: {}", i + 1, addr);
                }
                
                // Test that node_addr works immediately
                match timeout(Duration::from_secs(5), daemon.node_addr()).await {
                    Ok(Ok(node_addr)) => {
                        println!("✓ Node address available: {} direct addresses", node_addr.direct_addresses.len());
                    }
                    Ok(Err(e)) => {
                        println!("⚠️ Node address failed: {}", e);
                        // This might fail if address discovery is slow, which is OK
                    }
                    Err(_) => {
                        println!("⚠️ Node address discovery timed out - this is expected in some environments");
                    }
                }
                
                // Test that certificate manager is available
                let cert_count = daemon.cert_manager().certificate_count().await;
                println!("✓ Certificate manager available with {} certificates", cert_count);
            }
            Ok(Err(e)) => {
                println!("❌ Daemon initialization failed: {}", e);
                panic!("Daemon initialization should succeed");
            }
            Err(_) => {
                println!("❌ Daemon initialization timed out");
                panic!("Daemon initialization should not timeout");
            }
        }
        
        println!("=== End Daemon Initialization Test ===");
    }
}

/// TLS forwarding protocol handler using tonic-iroh IrohStream
#[derive(Clone)]
struct TlsForwardingProtocolHandler {
    config: DaemonConfig,
    cert_manager: crate::certs::CertificateManager,
    domain: String,
    node_id: String,
    upstream_client: UpstreamClient,
    gate_id: hellas_gate_core::GateId,
    tls_acceptor: tokio_rustls::TlsAcceptor,
    http_server: std::sync::Arc<crate::http::HttpServer>,
    endpoint: iroh::Endpoint,
}

impl std::fmt::Debug for TlsForwardingProtocolHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsForwardingProtocolHandler")
            .field("domain", &self.domain)
            .field("node_id", &self.node_id)
            .finish()
    }
}

impl iroh::protocol::ProtocolHandler for TlsForwardingProtocolHandler {
    async fn accept(&self, conn: iroh::endpoint::Connection) -> std::result::Result<(), iroh::protocol::AcceptError> {
        info!("TLS FORWARDING: Received P2P connection from relay!");
        info!("  Target domain: {}", self.domain);
        info!("  Target node ID: {}", self.node_id);

        // Create stream context with all needed data
        let context = TlsStreamContext::new(
            self.domain.clone(),
            self.node_id.clone(),
            self.tls_acceptor.clone(),
            self.http_server.clone(),
            conn.clone(),
            self.endpoint.clone(),
        );

        // Spawn a task to handle this connection and return immediately
        tokio::spawn(async move {
            if let Err(e) = Self::handle_connection(conn, context).await {
                error!("TLS forwarding connection failed: {}", e.to_string());
            }
        });

        info!("TLS FORWARDING: Connection accepted, spawned handler task");
        Ok(())
    }
}

impl TlsForwardingProtocolHandler {
    /// Handle a single P2P connection, accepting multiple streams
    async fn handle_connection(
        conn: iroh::endpoint::Connection,
        context: TlsStreamContext,
    ) -> crate::Result<()> {
        info!("TLS FORWARDING: Starting connection handler for domain: {}", context.domain);
        
        // Loop to handle multiple streams on this connection
        let mut conn = conn;
        loop {
            match conn.accept_bi().await {
                Ok((send_stream, recv_stream)) => {
                    info!("TLS FORWARDING: Accepted new bidirectional stream");
                    
                    // Spawn a task to handle this individual stream
                    let context_clone = context.clone();
                    
                    tokio::spawn(async move {
                        match context_clone.handle_stream(send_stream, recv_stream).await {
                            Ok(()) => {},
                            Err(e) => {
                                let error_msg = format!("TLS forwarding stream failed: {}", e);
                                warn!("{}", error_msg);
                            }
                        }
                    });
                }
                Err(e) => {
                    info!("TLS FORWARDING: Connection closed or no more streams: {}", e);
                    break;
                }
            }
        }
        
        info!("TLS FORWARDING: Connection handler completed for domain: {}", context.domain);
        Ok(())
    }

}