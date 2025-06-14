//! CLI commands

use anyhow::Result;
use clap::Subcommand;
use hellas_gate_core::{load_or_generate_identity, node_id_from_identity};
use hellas_gate_daemon::GateDaemon;
use hellas_gate_relay::RelayServer;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::info;

use crate::config;

#[derive(Subcommand)]
pub enum Commands {
    /// Start the Gate daemon
    Daemon {
        /// Daemon configuration file
        #[arg(long)]
        config: Option<PathBuf>,
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

    // Create daemon (this fully initializes and starts background services)
    let daemon = GateDaemon::new(config, identity, daemon_dir.clone()).await?;

    // Write peer address file for relay discovery
    let node_addr = daemon.node_addr().await?;
    let peer_addr_file = daemon_dir.join("peer_addr");
    std::fs::write(&peer_addr_file, node_addr.to_string())?;
    info!("Saved peer address to: {:?}", peer_addr_file);

    // Wait for daemon shutdown (this blocks until shutdown)
    daemon
        .wait_for_shutdown()
        .await
        .map_err(|e| anyhow::anyhow!("Daemon error: {}", e))
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

    // Create secret key from identity
    let key_array: [u8; 32] = identity[0..32]
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid identity key length"))?;
    let secret_key = iroh::SecretKey::from_bytes(&key_array);

    // Create endpoint
    let bind_addr = format!("0.0.0.0:{}", p2p_bind.port());
    let endpoint = iroh::Endpoint::builder()
        .secret_key(secret_key)
        .bind_addr_v4(bind_addr.parse()?)
        .relay_mode(iroh::RelayMode::Disabled)
        .discovery_n0()
        .discovery_local_network()
        .bind()
        .await?;

    info!("P2P endpoint bound to port {}", p2p_bind.port());

    // Create and start relay server
    let relay = RelayServer::new(config, endpoint).await?;
    info!("Relay server starting");

    // Run until shutdown signal
    relay.run().await?;

    info!("Relay server shutdown complete");
    Ok(())
}

async fn generate_certificate(
    node_id: Option<String>,
    identity: Option<PathBuf>,
    output: Option<PathBuf>,
    pem_only: bool,
    data_dir: PathBuf,
) -> Result<()> {
    use hellas_gate_daemon::certs::CertificateManager;

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
        // Derive node ID from private key using shared function
        node_id_from_identity(&private_key)
            .map_err(|e| anyhow::anyhow!("Failed to derive node ID: {}", e))?
    };

    info!("Generating certificate for node ID: {}", node_id_hex);

    // Create certificate manager and generate self-signed certificate
    let cert_dir = data_dir.join("certificates");
    let le_config = hellas_gate_daemon::LetsEncryptConfig::default();
    let cert_manager = CertificateManager::new(le_config, cert_dir)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create certificate manager: {}", e))?;
    let domain = format!("{}.private.hellas.ai", &node_id_hex[..16]); // Use first 16 chars like daemon

    // Use the public API to get or create certificate (which will fall back to self-signed)
    let cert_info = cert_manager
        .get_or_create_certificate(&domain, &node_id_hex, &private_key, None)
        .await
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
        println!("{}", cert_info.cert_pem);
        println!("=== PRIVATE KEY PEM ===");
        println!("{}", cert_info.key_pem);
    } else {
        // Save to files and show info
        let cert_file = output_dir.join(format!("{}.crt", node_id_hex));
        let key_file = output_dir.join(format!("{}.key", node_id_hex));

        std::fs::write(&cert_file, &cert_info.cert_pem)?;
        std::fs::write(&key_file, &cert_info.key_pem)?;

        println!("Certificate generated successfully!");
        println!("  Domain: {}", cert_info.domain);
        println!("  Certificate Type: {:?}", cert_info.cert_type);
        println!("  Certificate: {}", cert_file.display());
        println!("  Private Key: {}", key_file.display());
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
