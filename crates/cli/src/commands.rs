//! CLI commands

use anyhow::Result;
use clap::Subcommand;
use hellas_gate_daemon::{DaemonConfig, GateDaemon};
use hellas_gate_p2p::P2PSession;
use hellas_gate_relay::RelayServer;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::config;

/// Load or generate identity for a component
fn load_or_generate_identity(component_dir: &PathBuf) -> Result<Vec<u8>> {
    let identity_file = component_dir.join("identity.key");

    if identity_file.exists() {
        let key_data = std::fs::read(&identity_file)?;
        info!("Loaded identity from: {:?}", identity_file);
        Ok(key_data)
    } else {
        // Generate new identity and save it
        let mut rng = rand::thread_rng();
        let secret_key = iroh::SecretKey::generate(&mut rng);
        let key_bytes = secret_key.to_bytes();

        // Create directory and save the key
        std::fs::create_dir_all(component_dir)?;
        std::fs::write(&identity_file, &key_bytes)?;
        info!("Generated and saved new identity to: {:?}", identity_file);

        Ok(key_bytes.to_vec())
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the Gate daemon
    Daemon {
        /// Daemon configuration file
        #[arg(long)]
        config: Option<PathBuf>,
    },

    /// P2P networking commands
    P2p {
        /// Peer address to connect to (defaults to .peer_id file)
        #[arg(long)]
        peer: Option<String>,

        /// Private key file for identity (defaults to daemon's identity)
        #[arg(long)]
        identity: Option<PathBuf>,

        #[command(subcommand)]
        command: P2PCommands,
    },

    /// Start the relay server
    Relay {
        /// HTTPS bind address
        #[arg(long, default_value = "0.0.0.0:8443")]
        bind: SocketAddr,

        /// P2P interface bind address
        #[arg(long, default_value = "0.0.0.0:41146")]
        p2p_bind: SocketAddr,
    },

    /// Generate default configuration files
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// TLS certificate operations
    Cert {
        #[command(subcommand)]
        command: CertCommands,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Generate daemon configuration file
    Daemon {
        /// Output file path (defaults to HELLAS_STATE_DIR/daemon/daemon.json)
        output: Option<PathBuf>,
    },

    /// Generate relay configuration file
    Relay {
        /// Output file path (defaults to HELLAS_STATE_DIR/relay/relay.json)
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum P2PCommands {
    /// Connect to a peer and send an inference request
    Inference {
        /// Model name
        #[arg(long)]
        model: String,

        /// User message
        #[arg(long)]
        message: String,
    },

    /// List available models from a remote peer
    ListModels,

    /// Show node information
    Info,

    /// List connected peers
    Peers,
}

#[derive(Subcommand)]
pub enum CertCommands {
    /// Generate a self-signed TLS certificate
    Generate {
        /// Node ID (hex string) or use daemon's identity
        #[arg(long)]
        node_id: Option<String>,

        /// Private key file for identity (defaults to daemon's identity)
        #[arg(long)]
        identity: Option<PathBuf>,

        /// Output directory for certificate files
        #[arg(long)]
        output: Option<PathBuf>,

        /// Output in PEM format (default is both PEM and info)
        #[arg(long)]
        pem_only: bool,
    },

    /// Show certificate information
    Info {
        /// Certificate file path
        #[arg(long)]
        cert: PathBuf,
    },
}

impl Commands {
    pub async fn execute(self, data_dir: Option<PathBuf>) -> Result<()> {
        // Determine data directory with default fallback
        let data_dir = data_dir.unwrap_or_else(|| {
            // Check environment variable first, then fall back to system data dir
            if let Ok(gate_data_dir) = std::env::var("GATE_STATE_DIR") {
                PathBuf::from(gate_data_dir)
            } else {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("gate")
            }
        });

        match self {
            Commands::Daemon { config } => start_daemon(config, data_dir).await,
            Commands::P2p {
                peer,
                identity,
                command,
            } => command.execute(peer, identity, data_dir).await,
            Commands::Relay { bind, p2p_bind } => start_relay(bind, p2p_bind, data_dir).await,
            Commands::Config { command } => command.execute(data_dir).await,
            Commands::Cert { command } => command.execute(data_dir).await,
        }
    }
}

impl ConfigCommands {
    pub async fn execute(self, data_dir: PathBuf) -> Result<()> {
        match self {
            ConfigCommands::Daemon { output } => {
                let config_path = if let Some(path) = output {
                    path
                } else {
                    data_dir.join("daemon").join("daemon.json")
                };

                // Create parent directory if it doesn't exist
                if let Some(parent) = config_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                config::generate_default_daemon_config(&config_path)?;
                println!(
                    "Generated daemon configuration at: {}",
                    config_path.display()
                );
                Ok(())
            }
            ConfigCommands::Relay { output } => {
                let config_path = if let Some(path) = output {
                    path
                } else {
                    data_dir.join("relay").join("relay.json")
                };

                // Create parent directory if it doesn't exist
                if let Some(parent) = config_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                config::generate_default_relay_config(&config_path)?;
                println!(
                    "Generated relay configuration at: {}",
                    config_path.display()
                );
                Ok(())
            }
        }
    }
}

impl CertCommands {
    pub async fn execute(self, data_dir: PathBuf) -> Result<()> {
        match self {
            CertCommands::Generate {
                node_id,
                identity,
                output,
                pem_only,
            } => generate_certificate(node_id, identity, output, pem_only, data_dir).await,
            CertCommands::Info { cert } => show_certificate_info(cert).await,
        }
    }
}

impl P2PCommands {
    pub async fn execute(
        self,
        peer: Option<String>,
        identity: Option<PathBuf>,
        state_dir: PathBuf,
    ) -> Result<()> {
        // Create shared P2P session
        let session = create_p2p_session(identity, state_dir.clone()).await?;
        let node_id = session.node_id();
        info!("Started P2P session with node ID: {node_id}");

        // Execute specific command with the session
        match self {
            P2PCommands::Inference { model, message } => {
                let peer_addr = resolve_peer_address(peer, &state_dir).await?;
                send_inference_with_session(session, peer_addr, model, message).await
            }
            P2PCommands::ListModels => {
                let peer_addr = resolve_peer_address(peer, &state_dir).await?;
                list_models_with_session(session, peer_addr).await
            }
            P2PCommands::Info => show_info_with_session(session).await,
            P2PCommands::Peers => list_peers_with_session(session).await,
        }
    }
}

async fn start_daemon(config_file: Option<PathBuf>, data_dir: PathBuf) -> Result<()> {
    info!("Starting Gate daemon");

    // Set component-specific state directory via environment variable
    let daemon_dir = data_dir.join("daemon");
    std::env::set_var("GATE_STATE_DIR", &daemon_dir);

    // Load or generate daemon identity
    let identity = load_or_generate_identity(&daemon_dir)?;

    let config = if config_file.is_some() {
        info!(
            "Loading configuration from: {:?}",
            config_file.as_ref().unwrap()
        );
        config::load_daemon_config(config_file)?
    } else {
        // Check for default config file
        let default_config = daemon_dir.join("daemon.json");
        if default_config.exists() {
            info!("Loading configuration from: {:?}", default_config);
            config::load_daemon_config(Some(default_config))?
        } else {
            info!("Using default configuration with environment overrides");
            config::load_daemon_config(None::<PathBuf>)?
        }
    };

    let mut daemon = GateDaemon::new(config, identity, daemon_dir.clone())?;

    // Write peer address file for relay discovery
    let node_addr = daemon.run().await?;
    let peer_addr_file = daemon_dir.join("peer_addr");
    std::fs::write(&peer_addr_file, node_addr.to_string())?;
    info!("Saved peer address to: {:?}", peer_addr_file);

    Ok(())
}

/// Resolve peer address from CLI argument or daemon peer_addr file
async fn resolve_peer_address(peer: Option<String>, data_dir: &PathBuf) -> Result<String> {
    if let Some(peer_addr) = peer {
        return Ok(peer_addr);
    }

    // Try to read from daemon's peer_addr file
    let daemon_dir = data_dir.join("daemon");
    let peer_id_file = daemon_dir.join("peer_addr");

    if peer_id_file.exists() {
        let peer_addr = std::fs::read_to_string(&peer_id_file)?.trim().to_string();
        info!("Using peer address from daemon peer_addr file: {peer_addr}");
        Ok(peer_addr)
    } else {
        Err(anyhow::anyhow!(
            "No peer address provided and daemon peer_addr file not found. Use --peer flag or start daemon first."
        ))
    }
}

/// Create a P2P session with shared identity logic
async fn create_p2p_session(
    identity_file: Option<PathBuf>,
    data_dir: PathBuf,
) -> Result<P2PSession> {
    let mut builder = P2PSession::builder();

    // Determine identity to use
    let identity = if let Some(provided_path) = identity_file {
        if provided_path.exists() {
            let key_data = std::fs::read(&provided_path)?;
            info!("Loaded identity from: {:?}", provided_path);
            key_data
        } else {
            return Err(anyhow::anyhow!(
                "Identity file not found at {:?}",
                provided_path
            ));
        }
    } else {
        // Use CLI component identity
        let cli_dir = data_dir.join("cli");
        load_or_generate_identity(&cli_dir)?
    };

    builder = builder.with_private_key(&identity)?;

    builder
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("P2P session build failed: {e}"))
}

async fn send_inference_with_session(
    session: P2PSession,
    peer_addr: String,
    model: String,
    message: String,
) -> Result<()> {
    info!("Sending inference request to peer: {peer_addr}");

    // Parse and add peer
    let gate_addr: hellas_gate_core::GateAddr =
        peer_addr.parse().expect("Invalid peer address format");
    let connection_handle = session.add_peer(gate_addr).await?;
    let peer_id = connection_handle.wait_connected().await?;
    info!("Connected to peer: {peer_id}");

    // TODO: Implement inference request with new streaming API
    println!("Connected to peer successfully!");
    println!("Model: {}, Message: {}", model, message);

    Ok(())
}

async fn show_info_with_session(session: P2PSession) -> Result<()> {
    info!("Getting node information");

    let node_id = session.node_id();
    let node_addr = session.node_addr().await?;

    println!("Node ID: {node_id}");
    println!("Node Address: {node_addr}");

    Ok(())
}

async fn list_peers_with_session(session: P2PSession) -> Result<()> {
    info!("Listing connected peers");

    let peers = session.list_peers().await;

    if peers.is_empty() {
        println!("No connected peers");
    } else {
        println!("Connected peers:");
        for peer in peers {
            println!("  {peer}");
        }
    }

    Ok(())
}

async fn list_models_with_session(session: P2PSession, peer_addr: String) -> Result<()> {
    info!("Listing models from peer: {peer_addr}");

    // Parse and add peer
    let gate_addr: hellas_gate_core::GateAddr =
        peer_addr.parse().expect("Invalid peer address format");
    let connection_handle = session.add_peer(gate_addr).await?;
    let peer_id = connection_handle.wait_connected().await?;
    info!("Connected to peer: {peer_id}");

    // TODO: Implement model list request with new streaming API
    println!("Connected to peer successfully!");
    println!("List models functionality will be implemented with streaming API");

    Ok(())
}

async fn start_relay(bind: SocketAddr, p2p_bind: SocketAddr, data_dir: PathBuf) -> Result<()> {
    info!("Starting Gate relay server");

    // Set component-specific state directory via environment variable
    let relay_dir = data_dir.join("relay");
    std::env::set_var("GATE_STATE_DIR", &relay_dir);

    // Load or generate relay identity
    let identity = load_or_generate_identity(&relay_dir)?;

    // Load relay configuration (using CLI override for bind addresses)
    let default_config = relay_dir.join("relay.json");
    let mut config = if default_config.exists() {
        info!("Loading relay configuration from: {:?}", default_config);
        config::load_relay_config(Some(default_config))?
    } else {
        info!(
            "No relay configuration found, generating default at: {:?}",
            default_config
        );
        let config = config::load_relay_config(None::<PathBuf>)?;

        // Create relay directory and write default config
        std::fs::create_dir_all(&relay_dir)?;
        config::save_relay_config(&config, &default_config)?;
        info!("Generated default relay configuration");

        config
    };

    // Override config with CLI arguments
    config.https.bind_addr = bind;
    config.p2p.port = p2p_bind.port();

    info!("HTTPS bind address: {}", config.https.bind_addr);
    info!("P2P bind address: 0.0.0.0:{}", config.p2p.port);

    // Create and start relay server
    let relay = RelayServer::new(config, identity).await?;

    // Note: Do NOT auto-connect to local daemon
    // The daemon should connect TO the relay as a bootstrap peer, not the other way around
    // discover_and_add_local_daemon(&relay, &data_dir).await;

    info!("Relay server starting on {}", bind);

    // Run until shutdown signal
    relay.run().await?;

    info!("Relay server shutdown complete");
    Ok(())
}

async fn discover_and_add_local_daemon(relay: &RelayServer, data_dir: &PathBuf) {
    info!("Looking for local Gate daemon...");

    // Check daemon's data directory for peer info
    let daemon_dir = data_dir.join("daemon");
    let peer_id_file = daemon_dir.join("peer_addr");

    if peer_id_file.exists() {
        match std::fs::read_to_string(&peer_id_file) {
            Ok(peer_addr) => {
                let peer_addr = peer_addr.trim();
                info!("Found local daemon peer address: {}", peer_addr);

                // Extract node ID from peer address to generate proper domain
                match peer_addr.parse::<hellas_gate_core::GateAddr>() {
                    Ok(gate_addr) => {
                        let node_id_hex = hex::encode(gate_addr.id.as_bytes());
                        let domain = format!("{}.private.hellas.ai", node_id_hex);

                        match relay.add_peer(peer_addr, domain.clone()).await {
                            Ok(gate_id) => {
                                info!("Successfully connected to local daemon: {}", gate_id);
                                info!("Local daemon available at: {}", domain);
                            }
                            Err(e) => {
                                warn!("Failed to connect to local daemon: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse daemon peer address '{}': {}", peer_addr, e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read peer ID file: {}", e);
            }
        }
    } else {
        info!("No local daemon found (no .peer_id file)");
        info!("Start a Gate daemon first, then start the relay to enable local testing");
    }
}

async fn generate_certificate(
    node_id: Option<String>,
    identity: Option<PathBuf>,
    output: Option<PathBuf>,
    pem_only: bool,
    data_dir: PathBuf,
) -> Result<()> {
    use hellas_gate_daemon::selfsigned::TlsCertManager;

    // Load private key
    let identity_path = if let Some(path) = identity {
        path
    } else {
        data_dir.join("daemon").join("identity.key")
    };

    let private_key = if identity_path.exists() {
        std::fs::read(&identity_path)?
    } else {
        anyhow::bail!("Private key file not found: {}. Generate one using 'gate daemon' or specify --identity", identity_path.display());
    };

    // Determine node ID
    let node_id_hex = if let Some(id) = node_id {
        id
    } else {
        // Derive node ID from private key
        if private_key.len() == 32 {
            let key_array: [u8; 32] = private_key[0..32]
                .try_into()
                .map_err(|_| anyhow::anyhow!("Failed to convert key to array"))?;
            let secret_key = iroh::SecretKey::from_bytes(&key_array);
            hex::encode(secret_key.public().as_bytes())
        } else {
            anyhow::bail!(
                "Private key must be 32 bytes, got {} bytes",
                private_key.len()
            );
        }
    };

    info!("Generating certificate for node ID: {}", node_id_hex);

    // Generate certificate
    let cert_manager = TlsCertManager::generate_self_signed(&node_id_hex, &private_key)
        .map_err(|e| anyhow::anyhow!("Failed to generate certificate: {}", e))?;

    // Determine output directory
    let output_dir = if let Some(dir) = output {
        dir
    } else {
        data_dir.join("certs")
    };

    std::fs::create_dir_all(&output_dir)?;

    if pem_only {
        // Just output PEM to stdout
        println!("=== CERTIFICATE PEM ===");
        println!("{}", cert_manager.certificate_pem()?);
        println!("=== PRIVATE KEY PEM ===");
        println!("{}", cert_manager.private_key_pem());
    } else {
        // Save to files and show info
        let cert_file = output_dir.join(format!("{}.crt", node_id_hex));
        let key_file = output_dir.join(format!("{}.key", node_id_hex));

        std::fs::write(&cert_file, cert_manager.certificate_pem()?)?;
        std::fs::write(&key_file, cert_manager.private_key_pem())?;

        println!("Certificate generated successfully!");
        println!("  Domain: {}", cert_manager.domain());
        println!("  Certificate: {}", cert_file.display());
        println!("  Private Key: {}", key_file.display());
        println!("  {}", cert_manager.expiration_info());

        if cert_manager.is_expiring_soon() {
            println!("  ⚠️  Certificate will expire soon!");
        }
    }

    Ok(())
}

async fn show_certificate_info(cert_path: PathBuf) -> Result<()> {
    let cert_pem = std::fs::read_to_string(&cert_path)?;

    // For now, just show file info since parsing X.509 requires additional dependencies
    println!("Certificate file: {}", cert_path.display());
    println!("Size: {} bytes", cert_pem.len());

    if cert_pem.contains("-----BEGIN CERTIFICATE-----") {
        println!("Format: PEM");
        println!("Content preview:");
        let lines: Vec<&str> = cert_pem.lines().take(5).collect();
        for line in lines {
            println!("  {}", line);
        }
        if cert_pem.lines().count() > 5 {
            println!("  ... (truncated)");
        }
    } else {
        println!("Format: Unknown (not PEM)");
    }

    Ok(())
}
