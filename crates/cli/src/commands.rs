//! CLI commands

use anyhow::Result;
use clap::Subcommand;
use hellas_gate_daemon::{DaemonConfig, GateDaemon};
use hellas_gate_p2p::P2PSession;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::config;

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

    /// Generate a default configuration file
    Config {
        /// Output file path
        #[arg(long, default_value = "gate.json")]
        output: PathBuf,
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

impl Commands {
    pub async fn execute(self, config_file: Option<PathBuf>) -> Result<()> {
        match self {
            Commands::Daemon { config } => {
                let config_path = config.or(config_file);
                start_daemon(config_path).await
            }
            Commands::P2p {
                peer,
                identity,
                command,
            } => command.execute(peer, identity).await,
            Commands::Config { output } => generate_config(output).await,
        }
    }
}

impl P2PCommands {
    pub async fn execute(self, peer: Option<String>, identity: Option<PathBuf>) -> Result<()> {
        // Create shared P2P session
        let (session, _request_rx) = create_p2p_session(identity).await?;
        let node_id = session.node_id();
        info!("Started P2P session with node ID: {node_id}");

        // Execute specific command with the session
        match self {
            P2PCommands::Inference { model, message } => {
                let peer_addr = resolve_peer_address(peer).await?;
                send_inference_with_session(session, peer_addr, model, message).await
            }
            P2PCommands::ListModels => {
                let peer_addr = resolve_peer_address(peer).await?;
                list_models_with_session(session, peer_addr).await
            }
            P2PCommands::Info => show_info_with_session(session).await,
            P2PCommands::Peers => list_peers_with_session(session).await,
        }
    }
}

async fn start_daemon(config_file: Option<PathBuf>) -> Result<()> {
    info!("Starting Gate daemon");

    let config = if let Some(config_path) = config_file {
        info!("Loading configuration from: {:?}", config_path);
        DaemonConfig::from_file(config_path)?
    } else {
        info!("Using default configuration with environment overrides");
        DaemonConfig::from_env()?
    };

    let mut daemon = GateDaemon::new(config)?;
    daemon.run().await?;

    Ok(())
}

/// Resolve peer address from CLI argument or .peer_id file
async fn resolve_peer_address(peer: Option<String>) -> Result<String> {
    if let Some(peer_addr) = peer {
        return Ok(peer_addr);
    }

    // Try to read from .peer_id file
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gate");
    let peer_id_file = data_dir.join(".peer_id");

    if peer_id_file.exists() {
        let peer_addr = std::fs::read_to_string(&peer_id_file)?.trim().to_string();
        info!("Using peer address from .peer_id file: {peer_addr}");
        Ok(peer_addr)
    } else {
        Err(anyhow::anyhow!(
            "No peer address provided and .peer_id file not found. Use --peer flag or create .peer_id file."
        ))
    }
}

/// Create a P2P session with shared identity logic
async fn create_p2p_session(
    identity_file: Option<PathBuf>,
) -> Result<(
    P2PSession,
    tokio::sync::mpsc::UnboundedReceiver<hellas_gate_p2p::PendingRequest>,
)> {
    let mut builder = P2PSession::builder();

    // Determine identity file to use
    if let Some(provided_path) = identity_file {
        if provided_path.exists() {
            let key_data = std::fs::read(&provided_path)?;
            builder = builder.with_private_key(&key_data)?;
            info!("Loaded identity from: {:?}", provided_path);
        } else {
            warn!(
                "Identity file not found at {:?}, using random identity",
                provided_path
            );
        }
    } else {
        // CLI uses its own random identity (different from daemon)
        info!("Using random identity for CLI session");
    }

    builder
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("P2P session start failed: {e}"))
}

async fn send_inference_with_session(
    session: P2PSession,
    peer_addr: String,
    model: String,
    message: String,
) -> Result<()> {
    info!("Sending inference request to peer: {peer_addr}");

    // Connect to peer
    let peer_id = session.connect_str(&peer_addr).await?;
    info!("Connected to peer: {peer_id}");

    // Send inference request
    let request = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": message
            }
        ]
    });

    info!("Sending inference request...");
    let response = session.send_inference(peer_id, request).await?;

    // Print response
    println!("{}", serde_json::to_string_pretty(&response)?);

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

    // Connect to peer with timeout
    info!("Attempting to connect to peer...");
    let peer_id = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        session.connect_str(&peer_addr),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Connection timeout after 10 seconds"))?
    .map_err(|e| anyhow::anyhow!("Connection failed: {e}"))?;
    info!("Connected to peer: {peer_id}");

    // Send models list request
    let request = serde_json::json!({
        "action": "list_models"
    });

    info!("Requesting available models...");
    let response = session.send_inference(peer_id, request).await?;

    // Parse and display models
    if let Some(models) = response.get("models") {
        if let Some(models_array) = models.as_array() {
            println!("Available models from peer {peer_id}:");
            for model in models_array {
                if let Some(model_id) = model.get("id").and_then(|id| id.as_str()) {
                    println!("  - {model_id}");
                } else if let Some(model_str) = model.as_str() {
                    println!("  - {model_str}");
                }
            }
        } else {
            println!("Models response: {models}");
        }
    } else {
        println!(
            "Full response: {}",
            serde_json::to_string_pretty(&response)?
        );
    }

    Ok(())
}

async fn generate_config(output_path: PathBuf) -> Result<()> {
    info!("Generating default configuration: {:?}", output_path);

    config::generate_default_config(&output_path)?;

    println!(
        "Generated default configuration at: {}",
        output_path.display()
    );
    println!("Edit this file to customize your Gate daemon settings.");

    Ok(())
}
