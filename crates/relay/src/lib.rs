//! Gate Relay Server Library
//!
//! The relay server provides public HTTPS endpoints for Gate nodes by:
//! 1. Listening on :443 for TLS connections
//! 2. Extracting SNI from TLS ClientHello to identify target node
//! 3. Forwarding raw TLS bytes to target node via P2P connection
//! 4. Managing DNS records automatically

use futures::future::BoxFuture;
use futures::StreamExt;
use hellas_gate_proto::pb::gate::relay::v1::relay_service_server::RelayServiceServer;
use std::sync::Arc;
use tonic::transport::Server;
use tonic_iroh_transport::GrpcProtocolHandler;
use tracing::{error, info};

pub mod cloudflare;
pub mod config;
pub mod error;
pub mod https_proxy;
pub mod service;
pub mod stream;

pub use config::RelayConfig;
pub use error::RelayError;
pub use service::RelayServiceImpl;

// Re-export key types for convenience
pub use cloudflare::CloudflareDnsManager;
pub use https_proxy::{HttpsProxy, ProxyRegistry};
pub use stream::CombinedStream;

/// Simplified relay server using tonic-iroh transport
pub struct RelayServer {
    config: RelayConfig,
    endpoint: iroh::Endpoint,
    dns_manager: Arc<CloudflareDnsManager>,
    https_proxy: HttpsProxy,
}

impl RelayServer {
    /// Create a new relay server with config and an existing iroh endpoint
    pub async fn new(config: RelayConfig, endpoint: iroh::Endpoint) -> error::Result<Self> {
        info!("Initializing relay server");

        // Create components
        let dns_manager = Arc::new(CloudflareDnsManager::new().await?);
        let https_proxy = HttpsProxy::new(endpoint.clone());

        Ok(Self {
            config,
            endpoint,
            dns_manager,
            https_proxy,
        })
    }

    /// Get the HTTPS proxy registry for domain registration
    pub fn https_proxy_registry(&self) -> Arc<ProxyRegistry> {
        self.https_proxy.registry()
    }

    /// Start the relay server
    pub async fn run(self) -> error::Result<()> {
        info!("Starting Gate relay server");

        // Create relay service implementation
        let relay_service = Arc::new(RelayServiceImpl::new(
            self.dns_manager.clone(),
            self.https_proxy.registry(),
        ));

        // Set up tonic-iroh protocol handler
        let (handler, incoming, alpn) =
            GrpcProtocolHandler::for_service::<RelayServiceServer<RelayServiceImpl>>();

        info!(
            "Relay service started on protocol: {}",
            String::from_utf8_lossy(&alpn)
        );

        // Set up router
        let mut router = iroh::protocol::Router::builder(self.endpoint.clone());

        // Add relay service protocol
        router = router.accept(alpn, handler);

        // Add TLS forwarding protocol with opportunistic registration
        let tls_forward_alpn = b"gate-tls-forward".to_vec();
        let registry_handler = TlsForwardingHandler {
            registry: self.https_proxy.registry(),
        };
        router = router.accept(&tls_forward_alpn, registry_handler);

        let _router = router.spawn();

        // Start HTTPS proxy
        let https_proxy = self.https_proxy.clone();
        let https_bind_addr = self.config.https.bind_addr.to_string();
        let https_handle = tokio::spawn(async move {
            if let Err(e) = https_proxy.listen(&https_bind_addr).await {
                error!("HTTPS proxy error: {}", e);
            }
        });

        // Start tonic gRPC server with iroh incoming stream
        let relay_server_impl = (*relay_service).clone();
        let server_handle = tokio::spawn(async move {
            if let Err(e) = Server::builder()
                .add_service(RelayServiceServer::new(relay_server_impl))
                .serve_with_incoming(incoming)
                .await
            {
                error!("Relay gRPC server error: {}", e);
            }
        });

        // Start peer discovery for automatic registration
        let registry_clone = self.https_proxy.registry();
        let endpoint_clone = self.endpoint.clone();
        let discovery_handle = tokio::spawn(async move {
            Self::listen_for_peers(endpoint_clone, registry_clone).await;
        });

        info!("Relay server started successfully");
        info!("Node ID: {}", self.endpoint.node_id());
        info!("HTTPS bind address: {}", self.config.https.bind_addr);
        info!("P2P bind port: {}", self.config.p2p.port);

        // Wait for shutdown signal
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal");
            }
            result = server_handle => {
                if let Err(e) = result {
                    error!("gRPC server task error: {}", e);
                }
            }
            result = https_handle => {
                if let Err(e) = result {
                    error!("HTTPS proxy task error: {}", e);
                }
            }
            result = discovery_handle => {
                if let Err(e) = result {
                    error!("Peer discovery task error: {}", e);
                }
            }
        }

        info!("Relay server shutdown complete");
        Ok(())
    }

    /// Listen for peer discovery events and register all discovered peers
    async fn listen_for_peers(endpoint: iroh::Endpoint, registry: Arc<ProxyRegistry>) {
        info!("Starting peer discovery for automatic registration");

        // Track registered peers to avoid spam
        let mut registered_peers = std::collections::HashSet::new();

        // Get discovery stream from endpoint
        let mut discovery_stream = endpoint.discovery_stream();

        loop {
            match discovery_stream.next().await {
                Some(Ok(discovery_item)) => {
                    let peer_id = discovery_item.node_id();

                    // Check if already registered to prevent spam
                    if registered_peers.contains(&peer_id) {
                        continue;
                    }

                    info!(
                        "Discovered peer: {}, registering in proxy registry",
                        peer_id
                    );

                    // Register with full node address including direct addresses
                    let node_addr = discovery_item.to_node_addr();
                    registry.register_node_addr(node_addr).await;
                    registered_peers.insert(peer_id);
                }
                Some(Err(e)) => {
                    error!("Peer discovery error: {}", e);
                }
                None => {
                    info!("Peer discovery stream ended");
                    break;
                }
            }
        }
    }
}

/// Simple protocol handler for TLS forwarding that registers nodes
#[derive(Clone, Debug)]
struct TlsForwardingHandler {
    registry: Arc<ProxyRegistry>,
}

impl iroh::protocol::ProtocolHandler for TlsForwardingHandler {
    fn accept(&self, conn: iroh::endpoint::Connection) -> BoxFuture<'static, anyhow::Result<()>> {
        let registry = self.registry.clone();
        Box::pin(async move {
            // Register the connecting node opportunistically
            if let Ok(node_id) = conn.remote_node_id() {
                registry.register_node(node_id).await;
            }
            Ok(())
        })
    }
}
