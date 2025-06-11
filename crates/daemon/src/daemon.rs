//! Main Gate daemon implementation

use crate::config::DaemonConfig;
use crate::http::HttpServer;
use crate::upstream::UpstreamClient;
use crate::{DaemonError, Result};

use hellas_gate_p2p::{P2PSession, PendingRequest};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Main Gate daemon that orchestrates all services
pub struct GateDaemon {
    config: DaemonConfig,
    p2p_session: Option<P2PSession>,
    http_server: Option<HttpServer>,
    upstream_client: UpstreamClient,
    shutdown_token: CancellationToken,
}

impl GateDaemon {
    /// Create a new daemon with the given configuration
    ///
    /// # Errors
    ///
    /// Returns an error if daemon initialization fails
    pub fn new(config: DaemonConfig) -> Result<Self> {
        let upstream_client = UpstreamClient::new(&config.upstream)?;

        Ok(Self {
            config,
            p2p_session: None,
            http_server: None,
            upstream_client,
            shutdown_token: CancellationToken::new(),
        })
    }

    /// Start the daemon and all its services
    ///
    /// # Errors
    ///
    /// Returns an error if any service fails to start
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Gate daemon");

        // Create data directory if it doesn't exist
        if !self.config.data_dir.exists() {
            std::fs::create_dir_all(&self.config.data_dir)?;
            info!("Created data directory: {:?}", self.config.data_dir);
        }

        // Test upstream connection and list models
        self.test_upstream_connection().await?;

        // Start P2P session
        self.start_p2p().await?;

        // Start HTTP server
        self.start_http().await?;

        info!("Gate daemon started successfully");
        Ok(())
    }

    /// Test connection to upstream provider and list available models
    async fn test_upstream_connection(&self) -> Result<()> {
        info!(
            "Testing upstream connection to: {}",
            self.config.upstream.default_url
        );

        match self.upstream_client.list_models().await {
            Ok(models) => {
                info!("Upstream connection successful!");

                // Extract and log model names
                if let Some(data) = models.get("data").and_then(|d| d.as_array()) {
                    let model_names: Vec<String> = data
                        .iter()
                        .filter_map(|model| model.get("id").and_then(|id| id.as_str()))
                        .map(|s| s.to_string())
                        .collect();

                    info!("Available models: {:?}", model_names);
                    info!("Found {} models from upstream provider", model_names.len());

                    // Test with a simple inference request
                    self.test_simple_inference().await?;
                } else {
                    info!("Models response: {}", models);
                }
            }
            Err(e) => {
                warn!("Upstream connection failed: {}", e);
                warn!(
                    "Continuing without upstream provider - P2P requests will use mock responses"
                );
            }
        }

        Ok(())
    }

    /// Test a simple inference request to verify the loaded model works
    async fn test_simple_inference(&self) -> Result<()> {
        info!("Testing simple inference request");

        let test_request = crate::upstream::InferenceRequest::new(serde_json::json!({
            "model": "deepseek-r1-distill-qwen-32b-mlx",
            "messages": [
                {
                    "role": "user",
                    "content": "Respond with only the word 'SUCCESS' and nothing else."
                }
            ],
            "max_tokens": 5,
            "temperature": 0.1
        }))?;

        match self.upstream_client.chat_completion(test_request).await {
            Ok(response) => {
                if let Some(content) = response
                    .response
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|choice| choice.get("message"))
                    .and_then(|msg| msg.get("content"))
                    .and_then(|content| content.as_str())
                {
                    info!("Inference test successful! Response: '{}'", content.trim());
                } else {
                    info!("Inference test response: {}", response.response);
                }
            }
            Err(e) => {
                warn!("Inference test failed: {}", e);
                warn!("Model may not be loaded - continuing anyway");
            }
        }

        Ok(())
    }

    /// Start the P2P networking session
    async fn start_p2p(&mut self) -> Result<()> {
        info!("Starting P2P session");

        let mut builder = P2PSession::builder().with_port(self.config.p2p.port);

        // Load or generate identity
        let identity_file = self
            .config
            .p2p
            .identity_file
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.config.data_dir.join(".node_id"));

        if identity_file.exists() {
            let key_data = std::fs::read(&identity_file)?;
            builder = builder.with_private_key(&key_data)?;
            info!("Loaded P2P identity from: {:?}", identity_file);
        } else {
            // Generate new identity and save it
            let (new_builder, key_bytes) = builder.generate_identity_with_bytes();
            builder = new_builder;

            // Create parent directory if it doesn't exist
            if let Some(parent) = identity_file.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(&identity_file, key_bytes)?;
            info!(
                "Generated new P2P identity and saved to: {:?}",
                identity_file
            );
        }

        let (session, request_rx) = builder.start().await?;
        let node_id = session.node_id();
        let node_addr = session.node_addr().await?;

        info!("P2P session started with node ID: {node_id}");
        info!("P2P node address: {node_addr}");

        // Write peer address to .peer_id file for CLI to use
        let peer_id_file = self.config.data_dir.join(".peer_id");
        std::fs::write(&peer_id_file, node_addr.to_string())?;
        info!("Saved peer address to: {:?}", peer_id_file);

        // Connect to bootstrap peers
        for peer_addr_str in &self.config.p2p.bootstrap_peers {
            info!("Connecting to bootstrap peer: {peer_addr_str}");
            if let Err(e) = session.connect_str(peer_addr_str).await {
                warn!("Failed to connect to bootstrap peer {peer_addr_str}: {e}");
            }
        }

        // Start P2P request handler
        self.start_p2p_handler(request_rx).await?;

        self.p2p_session = Some(session);
        Ok(())
    }

    /// Start handler for incoming P2P requests
    async fn start_p2p_handler(
        &self,
        mut request_rx: mpsc::UnboundedReceiver<PendingRequest>,
    ) -> Result<()> {
        info!("Starting P2P request handler");

        let upstream_client = self.upstream_client.clone();
        let shutdown_token = self.shutdown_token.clone();

        // Spawn task to handle incoming P2P requests
        tokio::spawn(async move {
            info!("P2P request handler started");

            loop {
                tokio::select! {
                    request = request_rx.recv() => {
                        match request {
                            Some(pending_request) => {
                                let peer_id = pending_request.peer_id();
                                info!("Processing P2P request from peer: {peer_id}");

                                // Check if this is a special action request
                                if let Some(action) = pending_request.payload().get("action") {
                                    match action.as_str() {
                                        Some("list_models") => {
                                            info!("Handling list_models request from peer {peer_id}");

                                            // Get models from upstream provider
                                            match upstream_client.list_models().await {
                                                Ok(models_response) => {
                                                    let response = serde_json::json!({
                                                        "models": models_response.get("data")
                                                    });

                                                    if let Err(e) = pending_request.respond(response) {
                                                        warn!("Failed to send models response to peer {peer_id}: {e}");
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!("Failed to get models from upstream for peer {peer_id}: {e}");

                                                    let error_response = serde_json::json!({
                                                        "error": {
                                                            "message": "Failed to retrieve models",
                                                            "type": "upstream_error"
                                                        }
                                                    });

                                                    if let Err(e) = pending_request.respond(error_response) {
                                                        warn!("Failed to send error response to peer {peer_id}: {e}");
                                                    }
                                                }
                                            }
                                        }
                                        _ => {
                                            warn!("Unknown action '{}' from peer {peer_id}", action);

                                            let error_response = serde_json::json!({
                                                "error": {
                                                    "message": "Unknown action",
                                                    "type": "request_error"
                                                }
                                            });

                                            if let Err(e) = pending_request.respond(error_response) {
                                                warn!("Failed to send error response to peer {peer_id}: {e}");
                                            }
                                        }
                                    }
                                } else {
                                    // Try to forward regular inference request to upstream
                                    match crate::upstream::InferenceRequest::new(pending_request.payload().clone()) {
                                        Ok(inference_request) => {
                                            match upstream_client.chat_completion(inference_request).await {
                                                Ok(upstream_response) => {
                                                    if let Err(e) = pending_request.respond(upstream_response.response) {
                                                        warn!("Failed to send response to peer {peer_id}: {e}");
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!("Upstream request failed for peer {peer_id}: {e}");

                                                    // Send error response
                                                    let error_response = serde_json::json!({
                                                        "error": {
                                                            "message": "Upstream provider unavailable",
                                                            "type": "upstream_error"
                                                        }
                                                    });

                                                    if let Err(e) = pending_request.respond(error_response) {
                                                        warn!("Failed to send error response to peer {peer_id}: {e}");
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Invalid request from peer {peer_id}: {e}");

                                            // Send error response
                                            let error_response = serde_json::json!({
                                                "error": {
                                                    "message": "Invalid request format",
                                                    "type": "request_error"
                                                }
                                            });

                                            if let Err(e) = pending_request.respond(error_response) {
                                                warn!("Failed to send error response to peer {peer_id}: {e}");
                                            }
                                        }
                                    }
                                }
                            }
                            None => {
                                info!("P2P request channel closed");
                                break;
                            }
                        }
                    }
                    _ = shutdown_token.cancelled() => {
                        info!("P2P request handler shutting down");
                        break;
                    }
                }
            }

            info!("P2P request handler stopped");
        });

        Ok(())
    }

    /// Start the HTTP server
    async fn start_http(&mut self) -> Result<()> {
        info!("Starting HTTP server on {}", self.config.http.bind_addr);

        let p2p_session = self
            .p2p_session
            .as_ref()
            .ok_or_else(|| DaemonError::Other(anyhow::anyhow!("P2P session not started")))?;

        let http_server = HttpServer::new(
            self.config.http.clone(),
            Arc::new(self.upstream_client.clone()),
            p2p_session.node_id(),
        )?;

        self.http_server = Some(http_server);
        Ok(())
    }

    /// Run the daemon until shutdown
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon encounters a fatal error
    pub async fn run(&mut self) -> Result<()> {
        // Start the daemon
        self.start().await?;

        // Wait for shutdown signal
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down");
            }
            _ = self.shutdown_token.cancelled() => {
                info!("Received shutdown signal");
            }
        }

        // Shutdown gracefully
        self.shutdown().await
    }

    /// Gracefully shutdown the daemon
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown fails
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Gate daemon");

        // Signal shutdown
        self.shutdown_token.cancel();

        // Shutdown HTTP server
        if let Some(mut http_server) = self.http_server.take() {
            if let Err(e) = http_server.shutdown().await {
                warn!("Error shutting down HTTP server: {e}");
            }
        }

        // Shutdown P2P session
        if let Some(mut p2p_session) = self.p2p_session.take() {
            if let Err(e) = p2p_session.shutdown().await {
                warn!("Error shutting down P2P session: {e}");
            }
        }

        info!("Gate daemon shutdown complete");
        Ok(())
    }

    /// Get the daemon configuration
    #[must_use]
    pub const fn config(&self) -> &DaemonConfig {
        &self.config
    }

    /// Check if the daemon is shutting down
    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_token.is_cancelled()
    }
}
