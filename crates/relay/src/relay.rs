use crate::{
    cloudflare_dns::CloudflareDnsChallengeHandler,
    config::RelayConfig,
    dns::DnsManager,
    error::{RelayError, Result},
    registry::NodeRegistry,
    sni::SniExtractor,
    tls_proxy::TlsProxy,
};
use hellas_gate_core::{GateAddr, GateId};
use hellas_gate_p2p::P2PSession;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

/// Main relay server that coordinates all relay functionality
pub struct RelayServer {
    /// Relay configuration
    config: RelayConfig,

    /// TCP listener for incoming HTTPS connections
    listener: TcpListener,

    /// P2P session for communicating with Gate nodes
    p2p_session: Arc<P2PSession>,

    /// Registry of active nodes and their domain mappings
    node_registry: Arc<NodeRegistry>,

    /// DNS manager for domain provisioning
    dns_manager: Arc<DnsManager>,

    /// SNI extractor for identifying target nodes
    sni_extractor: SniExtractor,

    /// TLS proxy for forwarding raw TLS bytes
    tls_proxy: Arc<TlsProxy>,
}

impl RelayServer {
    /// Create a new relay server with configuration and identity
    pub async fn new(config: RelayConfig, identity: Vec<u8>) -> Result<Self> {
        info!("Initializing relay server on {}", config.https.bind_addr);

        // Bind to HTTPS port
        let listener = TcpListener::bind(config.https.bind_addr).await?;

        // Create components first
        let dns_manager = Arc::new(DnsManager::new().await?);
        let cloudflare_handler = Arc::new(CloudflareDnsChallengeHandler::new(dns_manager.clone()));

        // Initialize P2P session for relay communication
        let mut builder = P2PSession::builder()
            .with_port(config.p2p.port)
            .with_sni_proxy() // Enable SNI proxy protocol for relay
            .with_dns_challenge() // Enable DNS challenge support for ACME
            .with_dns_challenge_handler(cloudflare_handler); // Set Cloudflare DNS handler
        builder = builder
            .with_private_key(&identity)
            .map_err(|e| RelayError::P2P(e))?;

        let mut p2p_session = builder.build().await.map_err(|e| RelayError::P2P(e))?;

        // Extract SNI proxy handle for TLS proxy
        let sni_proxy_handle = p2p_session
            .take_sni_proxy_handle()
            .ok_or_else(|| RelayError::Config("SNI proxy not enabled".to_string()))?;

        let p2p_session = Arc::new(p2p_session);

        // Create remaining components
        let node_registry = Arc::new(NodeRegistry::new());
        let sni_extractor = SniExtractor::new();
        let tls_proxy = Arc::new(TlsProxy::new(
            p2p_session.clone(),
            node_registry.clone(),
            sni_proxy_handle,
        ));

        Ok(Self {
            config,
            listener,
            p2p_session,
            node_registry,
            dns_manager,
            sni_extractor,
            tls_proxy,
        })
    }

    /// Get the relay's node information for other nodes to connect to
    pub async fn node_addr(&self) -> Result<hellas_gate_core::GateAddr> {
        self.p2p_session
            .node_addr()
            .await
            .map_err(|e| RelayError::P2P(e))
    }

    /// Get relay's public IP addresses from P2P node
    pub async fn public_addresses(&self) -> Result<Vec<std::net::IpAddr>> {
        let node_addr = self
            .p2p_session
            .node_addr()
            .await
            .map_err(|e| RelayError::P2P(e))?;

        let mut addresses = Vec::new();

        // Extract IP addresses from direct_addresses socket addresses
        for socket_addr in &node_addr.direct_addresses {
            addresses.push(socket_addr.ip());
        }

        if addresses.is_empty() {
            return Err(RelayError::Config(
                "No public IP addresses found for relay node - cannot create DNS records"
                    .to_string(),
            ));
        }

        info!("Relay public addresses: {:?}", addresses);
        Ok(addresses)
    }

    /// Connect to a peer and register it with a domain
    pub async fn add_peer(&self, peer_addr: &str, domain: String) -> Result<GateId> {
        info!("Connecting to peer: {} with domain: {}", peer_addr, domain);

        let gate_addr: GateAddr = peer_addr.parse()?;
        let gate_id = gate_addr.id;

        // Add peer to P2P session (establishes persistent connection)
        let connection_handle = self.p2p_session.add_peer(gate_addr.clone()).await?;

        // Wait for connection to be established
        connection_handle.wait_connected().await?;

        // Register in node registry for domain mapping
        self.node_registry
            .register_node(gate_addr, domain.clone())
            .await?;

        info!("Successfully added peer {} with domain {}", gate_id, domain);
        Ok(gate_id)
    }

    /// Register a domain for a peer node and create DNS records
    pub async fn register_domain_for_peer(&self, peer_node_id: GateId) -> Result<String> {
        info!("Registering domain for peer node: {}", peer_node_id);

        // Get relay's public addresses
        let public_addresses = self.public_addresses().await?;

        // Provision subdomain with relay's public addresses
        let domain = self
            .dns_manager
            .provision_subdomain(peer_node_id, &public_addresses)
            .await?;

        // Register the domain mapping in the node registry
        // Create a GateAddr for the peer (we only have the node ID, so use placeholder for addresses)
        let peer_addr = GateAddr::new(peer_node_id, vec![]);

        self.node_registry
            .register_node(peer_addr, domain.clone())
            .await?;

        info!(
            "Successfully registered domain {} for peer {}",
            domain, peer_node_id
        );
        Ok(domain)
    }

    /// Run the relay server until shutdown
    pub async fn run(self) -> Result<()> {
        info!("Relay server starting...");

        // Start background tasks
        let registry_handle = {
            let registry = self.node_registry.clone();
            tokio::spawn(async move {
                registry.start_cleanup_task().await;
            })
        };

        // Main connection handling loop
        loop {
            tokio::select! {
                // Handle incoming HTTPS connections
                result = self.listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            let proxy = self.tls_proxy.clone();
                            let extractor = self.sni_extractor.clone();

                            tokio::spawn(async move {
                                if let Err(e) = proxy.handle_connection(stream, addr, extractor).await {
                                    warn!("Connection handling failed: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }

                // Handle shutdown signal
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutdown signal received");
                    break;
                }
            }
        }

        // Cleanup
        registry_handle.abort();
        info!("Relay server stopped");

        Ok(())
    }
}
