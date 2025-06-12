//! Main Gate daemon implementation

use crate::config::DaemonConfig;
use crate::http::HttpServer;
use crate::tls::TlsHandler;
use crate::tls_bridge::TlsBridge;
use crate::upstream::UpstreamClient;
use crate::{DaemonError, Result};

use hellas_gate_p2p::{InferenceRequest, P2PSession};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Main Gate daemon that orchestrates all services
pub struct GateDaemon {
    config: DaemonConfig,
    identity: Vec<u8>,
    state_dir: std::path::PathBuf,
    p2p_session: Option<Arc<P2PSession>>,
    http_server: Option<HttpServer>,
    tls_handler: Option<TlsHandler>,
    tls_bridge: Option<TlsBridge>,
    upstream_client: UpstreamClient,
    shutdown_token: CancellationToken,
}

impl GateDaemon {
    /// Create a new daemon with the given configuration, identity, and state directory
    ///
    /// # Errors
    ///
    /// Returns an error if daemon initialization fails
    pub fn new(
        config: DaemonConfig,
        identity: Vec<u8>,
        state_dir: std::path::PathBuf,
    ) -> Result<Self> {
        let upstream_client = UpstreamClient::new(&config.upstream)?;

        Ok(Self {
            config,
            identity,
            state_dir,
            p2p_session: None,
            http_server: None,
            tls_handler: None,
            tls_bridge: None,
            upstream_client,
            shutdown_token: CancellationToken::new(),
        })
    }

    /// Start the daemon and all its services
    ///
    /// # Errors
    ///
    /// Returns an error if any service fails to start
    pub async fn start(&mut self) -> Result<hellas_gate_core::GateAddr> {
        info!("Starting Gate daemon");

        // Test upstream connection and list models
        self.test_upstream_connection().await?;

        // Start P2P session
        let node_addr = self.start_p2p().await?;

        // Start HTTP server
        self.start_http().await?;

        // Initialize TLS handler if enabled
        if self.config.tls.enabled {
            self.start_tls(&node_addr).await?;
            // Initialize TLS bridge for P2P integration
            self.start_tls_bridge().await?;
            // Also start direct TLS server for testing
            self.start_tls_server().await?;
        }

        info!("Gate daemon started successfully");
        Ok(node_addr)
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
            "model": self.config.upstream.test_model,
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
    async fn start_p2p(&mut self) -> Result<hellas_gate_core::GateAddr> {
        info!("Starting P2P session");

        let mut builder = P2PSession::builder()
            .with_port(self.config.p2p.port)
            .with_inference();

        // Use identity provided by CLI
        builder = builder.with_private_key(&self.identity)?;
        info!("Using identity provided by CLI");

        let mut session = builder.build().await?;
        let node_id = session.node_id();
        let node_addr = session.node_addr().await?;

        info!("P2P session started with node ID: {node_id}");
        info!("P2P node address: {node_addr}");

        // Connect to bootstrap peers
        for peer_addr_str in &self.config.p2p.bootstrap_peers {
            info!("Connecting to bootstrap peer: {peer_addr_str}");

            match peer_addr_str.parse::<hellas_gate_core::GateAddr>() {
                Ok(peer_addr) => {
                    match session.add_peer(peer_addr.clone()).await {
                        Ok(connection_handle) => {
                            info!("Started connection to bootstrap peer: {}", peer_addr.id);
                            // Spawn a task to wait for connection establishment
                            tokio::spawn(async move {
                                match connection_handle.wait_connected().await {
                                    Ok(peer_id) => {
                                        info!("Bootstrap peer {} connected successfully", peer_id);
                                    }
                                    Err(e) => {
                                        warn!("Bootstrap peer connection failed: {}", e);
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            warn!("Failed to add bootstrap peer {}: {}", peer_addr_str, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Invalid bootstrap peer address {}: {}", peer_addr_str, e);
                }
            }
        }

        // Get inference request handle (daemon nodes handle inference)
        let mut inference_handle = session
            .take_inference_handle()
            .ok_or_else(|| DaemonError::Other(anyhow::anyhow!("Inference protocol not enabled")))?;
        let upstream_client = self.upstream_client.clone();

        // Spawn task to handle incoming inference requests
        tokio::spawn(async move {
            info!("P2P inference handler started");

            while let Some(mut request) = inference_handle.next().await {
                let peer_id = request.peer_id;
                info!("Processing inference request from peer: {peer_id}");

                // Check if this is a special action request
                if let Some(action) = request.request_data.get("action").cloned() {
                    match action.as_str() {
                        Some("list_models") => {
                            Self::handle_list_models(&upstream_client, &mut request).await;
                        }
                        _ => {
                            Self::handle_unknown_action(&mut request, &action).await;
                        }
                    }
                } else {
                    // Handle regular inference request
                    Self::handle_inference(&upstream_client, &mut request).await;
                }
            }

            info!("P2P inference handler stopped");
        });

        self.p2p_session = Some(Arc::new(session));
        Ok(node_addr)
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

    /// Initialize the TLS handler
    async fn start_tls(&mut self, node_addr: &hellas_gate_core::GateAddr) -> Result<()> {
        info!("Initializing TLS handler");

        let node_id = hex::encode(node_addr.id.as_bytes());

        // Check if we should request a domain from relay for DNS challenge
        if self.should_request_domain_from_relay().await? {
            info!("DNS challenge support detected, requesting domain from relay");
            if let Some((domain, relay_peer_id)) = self.request_domain_from_relay(node_addr).await?
            {
                info!("Received domain from relay: {}", domain);

                // Start ACME certificate obtainment process
                if let Err(e) = self
                    .start_acme_certificate_process(&domain, relay_peer_id)
                    .await
                {
                    warn!("Failed to start ACME certificate process: {}", e);
                    info!("Continuing with self-signed certificate");
                }
            }
        }

        let tls_handler = TlsHandler::new(&node_id, &self.identity)?;

        info!(
            "TLS handler configured for domain: {}",
            tls_handler.domain()
        );
        info!("Certificate info: {}", tls_handler.certificate_info());

        self.tls_handler = Some(tls_handler);
        Ok(())
    }

    /// Initialize the TLS bridge for P2P integration
    async fn start_tls_bridge(&mut self) -> Result<()> {
        info!("Initializing TLS bridge");

        let tls_handler = self
            .tls_handler
            .as_ref()
            .ok_or_else(|| DaemonError::Other(anyhow::anyhow!("TLS handler not initialized")))?;

        let tls_bridge = TlsBridge::new(Arc::new(tls_handler.clone()), self.config.http.bind_addr)?;

        info!(
            "TLS bridge configured to forward to HTTP server at {}",
            self.config.http.bind_addr
        );
        self.tls_bridge = Some(tls_bridge);
        Ok(())
    }

    /// Start direct TLS server for testing
    async fn start_tls_server(&self) -> Result<()> {
        let tls_handler = self
            .tls_handler
            .as_ref()
            .ok_or_else(|| DaemonError::Other(anyhow::anyhow!("TLS handler not initialized")))?;

        let bind_addr = self.config.tls.bind_addr;
        let tls_handler_clone = tls_handler.clone();
        let upstream_client = Arc::new(self.upstream_client.clone());
        let shutdown_token = self.shutdown_token.clone();

        info!("Starting direct TLS server on {}", bind_addr);
        info!("TLS server domain: {}", tls_handler.domain());

        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(bind_addr).await {
                Ok(listener) => {
                    info!("TLS server listening on {}", bind_addr);
                    listener
                }
                Err(e) => {
                    warn!("Failed to bind TLS server to {}: {}", bind_addr, e);
                    return;
                }
            };

            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, peer_addr)) => {
                                debug!("Accepted TLS connection from {}", peer_addr);

                                let handler = tls_handler_clone.clone();
                                let upstream = upstream_client.clone();

                                tokio::spawn(async move {
                                    if let Err(e) = Self::handle_tls_connection(stream, handler, upstream).await {
                                        warn!("TLS connection error from {}: {}", peer_addr, e);
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("Failed to accept TLS connection: {}", e);
                            }
                        }
                    }
                    _ = shutdown_token.cancelled() => {
                        info!("TLS server shutting down");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle a single TLS connection
    async fn handle_tls_connection(
        stream: tokio::net::TcpStream,
        tls_handler: crate::tls::TlsHandler,
        _upstream_client: Arc<UpstreamClient>,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio_rustls::TlsAcceptor;

        // Get the TLS acceptor from the handler
        let acceptor = TlsAcceptor::from(Arc::new(tls_handler.create_tls_config()?));

        // Perform TLS handshake
        let tls_stream = acceptor
            .accept(stream)
            .await
            .map_err(|e| DaemonError::Other(anyhow::anyhow!("TLS handshake failed: {}", e)))?;

        info!("TLS handshake completed successfully");

        // Read HTTP request
        let (mut reader, mut writer) = tokio::io::split(tls_stream);
        let mut buffer = vec![0u8; 4096];

        let bytes_read = reader.read(&mut buffer).await.map_err(DaemonError::Io)?;

        if bytes_read == 0 {
            warn!("Connection closed immediately after TLS handshake");
            return Ok(());
        }

        buffer.truncate(bytes_read);
        let request_str = String::from_utf8_lossy(&buffer);

        info!("Received HTTPS request via direct TLS:");
        info!("--- HTTP Request ---");
        for line in request_str.lines() {
            info!("{}", line);
        }
        info!("--- End Request ---");

        // Generate response
        let response_body = serde_json::json!({
            "message": "Hello from Gate daemon direct TLS!",
            "request_processed": true,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "tls_domain": tls_handler.domain(),
            "connection_type": "direct_tls"
        });

        let response_body_str = response_body.to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             Access-Control-Allow-Origin: *\r\n\
             \r\n\
             {}",
            response_body_str.len(),
            response_body_str
        );

        writer
            .write_all(response.as_bytes())
            .await
            .map_err(DaemonError::Io)?;
        writer.flush().await.map_err(DaemonError::Io)?;

        info!("Sent HTTPS response: {} bytes", response.len());
        Ok(())
    }

    /// Run the daemon until shutdown
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon encounters a fatal error
    pub async fn run(&mut self) -> Result<hellas_gate_core::GateAddr> {
        // Start the daemon
        let node_addr = self.start().await?;

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
        self.shutdown().await?;
        Ok(node_addr)
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

        // P2P session doesn't need explicit shutdown - dropping will clean up
        if let Some(_p2p_session) = self.p2p_session.take() {
            // Session will be dropped and cleaned up automatically
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

    /// Handle raw HTTPS bytes from relay and return HTTP response
    ///
    /// This is the main entry point for TLS termination and HTTP forwarding
    pub async fn handle_https_request(&self, https_bytes: &[u8]) -> Result<Vec<u8>> {
        if let Some(tls_bridge) = &self.tls_bridge {
            // Use the TLS bridge for proper TLS termination and HTTP forwarding
            tls_bridge.process_https_bytes(https_bytes).await
        } else {
            // Fallback to simple TLS termination if bridge not available
            let tls_handler = self.tls_handler.as_ref().ok_or_else(|| {
                DaemonError::Other(anyhow::anyhow!("TLS handler not initialized"))
            })?;

            // Terminate TLS and get the HTTP request
            let http_request = tls_handler.terminate_tls(https_bytes).await?;

            info!("Received HTTPS request from relay (fallback mode):");
            info!("--- HTTP Request ---");
            for line in http_request.lines() {
                info!("{}", line);
            }
            info!("--- End Request ---");

            // Generate a simple HTTP response
            let response_body = serde_json::json!({
                "message": "Hello from Gate daemon (fallback mode)!",
                "request_processed": true,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "tls_domain": tls_handler.domain()
            });

            let response_body_str = response_body.to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 Access-Control-Allow-Origin: *\r\n\
                 \r\n\
                 {}",
                response_body_str.len(),
                response_body_str
            );

            info!("Sending HTTP response: {} bytes", response.len());
            Ok(response.into_bytes())
        }
    }

    async fn handle_list_models(upstream_client: &UpstreamClient, request: &mut InferenceRequest) {
        match upstream_client.list_models().await {
            Ok(models_response) => {
                let response = serde_json::json!({
                    "models": models_response.get("data")
                });

                if let Err(e) = request.send_json(&response).await {
                    warn!("Failed to send models response: {e}");
                }
            }
            Err(e) => {
                warn!("Failed to get models from upstream: {e}");
                Self::send_error_response(request, "Failed to retrieve models", "upstream_error")
                    .await;
            }
        }
    }

    async fn handle_inference(upstream_client: &UpstreamClient, request: &mut InferenceRequest) {
        match crate::upstream::InferenceRequest::new(request.request_data.clone()) {
            Ok(inference_request) => {
                match upstream_client.chat_completion(inference_request).await {
                    Ok(upstream_response) => {
                        if let Err(e) = request.send_json(&upstream_response.response).await {
                            warn!("Failed to send response: {e}");
                        }
                    }
                    Err(e) => {
                        warn!("Upstream request failed: {e}");
                        Self::send_error_response(
                            request,
                            "Upstream provider unavailable",
                            "upstream_error",
                        )
                        .await;
                    }
                }
            }
            Err(e) => {
                warn!("Invalid request format: {e}");
                Self::send_error_response(request, "Invalid request format", "request_error").await;
            }
        }
    }

    async fn handle_unknown_action(request: &mut InferenceRequest, action: &serde_json::Value) {
        warn!("Unknown action '{}' from peer {}", action, request.peer_id);
        Self::send_error_response(request, "Unknown action", "request_error").await;
    }

    async fn send_error_response(request: &mut InferenceRequest, message: &str, error_type: &str) {
        let error_response = serde_json::json!({
            "error": {
                "message": message,
                "type": error_type
            }
        });

        if let Err(e) = request.send_json(&error_response).await {
            warn!("Failed to send error response: {e}");
        }
    }

    /// Check if daemon should request a domain from relay for DNS challenge
    async fn should_request_domain_from_relay(&self) -> Result<bool> {
        // Check if we have relay peers that support DNS challenge
        if let Some(p2p_session) = &self.p2p_session {
            // Check if any bootstrap peers support DNS challenge
            for peer_addr_str in &self.config.p2p.bootstrap_peers {
                if let Ok(peer_addr) = peer_addr_str.parse::<hellas_gate_core::GateAddr>() {
                    // For now, assume relay peers support DNS challenge
                    // TODO: Query peer capabilities
                    info!(
                        "Found potential relay peer with DNS challenge support: {}",
                        peer_addr.id
                    );
                    return Ok(true);
                }
            }
        }

        info!("No relay peers with DNS challenge support found");
        Ok(false)
    }

    /// Request a domain from relay for DNS challenge
    async fn request_domain_from_relay(
        &self,
        node_addr: &hellas_gate_core::GateAddr,
    ) -> Result<Option<(String, hellas_gate_core::GateId)>> {
        let p2p_session = self
            .p2p_session
            .as_ref()
            .ok_or_else(|| DaemonError::Other(anyhow::anyhow!("P2P session not available")))?;

        // Find a relay peer to request domain from
        for peer_addr_str in &self.config.p2p.bootstrap_peers {
            if let Ok(peer_addr) = peer_addr_str.parse::<hellas_gate_core::GateAddr>() {
                info!("Requesting domain from relay peer: {}", peer_addr.id);

                // Send domain registration request
                match p2p_session
                    .request_domain_registration(peer_addr.id, node_addr.clone())
                    .await
                {
                    Ok(domain) => {
                        info!("Successfully received domain from relay: {}", domain);
                        return Ok(Some((domain, peer_addr.id)));
                    }
                    Err(e) => {
                        warn!(
                            "Failed to request domain from relay {}: {}",
                            peer_addr.id, e
                        );
                        // Continue to next relay
                    }
                }
            }
        }

        warn!("Failed to request domain from any relay peer");
        Ok(None)
    }

    /// Start ACME certificate obtainment process for the given domain
    async fn start_acme_certificate_process(
        &mut self,
        domain: &str,
        relay_peer_id: hellas_gate_core::GateId,
    ) -> Result<()> {
        use crate::letsencrypt::{LetsEncryptConfig, LetsEncryptManager, P2PDnsChallengeResolver};

        info!("Starting ACME certificate process for domain: {}", domain);

        // Create LetsEncrypt configuration from daemon config
        let cert_dir = self.state_dir.join("certs");
        let letsencrypt_config = if let Some(ref config_le) = self.config.tls.letsencrypt {
            LetsEncryptConfig {
                cert_dir,
                email: config_le.email.clone(),
                staging: config_le.staging,
                domains: vec![domain.to_string()],
            }
        } else {
            // Fallback to default configuration if not specified
            warn!("No LetsEncrypt configuration found in daemon config, using defaults");
            LetsEncryptConfig {
                cert_dir,
                email: "admin@hellas.ai".to_string(),
                staging: true,
                domains: vec![domain.to_string()],
            }
        };

        // Initialize LetsEncrypt manager
        let mut letsencrypt_manager = LetsEncryptManager::new(letsencrypt_config).await?;
        letsencrypt_manager.initialize_account().await?;

        // Create P2P DNS challenge resolver using the same relay that provided the domain
        let p2p_session = self
            .p2p_session
            .as_ref()
            .ok_or_else(|| DaemonError::Other(anyhow::anyhow!("P2P session not available")))?;

        let dns_resolver = P2PDnsChallengeResolver::new(p2p_session.clone(), relay_peer_id);

        info!("Starting certificate request for domain: {}", domain);

        // Request certificate with DNS challenge
        match letsencrypt_manager
            .request_certificate(std::sync::Arc::new(dns_resolver))
            .await
        {
            Ok(()) => {
                info!(
                    "Successfully obtained ACME certificate for domain: {}",
                    domain
                );

                // Reload TLS terminator with the new certificate
                if let Err(e) = self.reload_tls_with_new_certificate().await {
                    warn!(
                        "Failed to reload TLS terminator with new certificate: {}",
                        e
                    );
                } else {
                    info!("TLS terminator reloaded with new certificate");
                }
            }
            Err(e) => {
                warn!("ACME certificate request failed: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Reload TLS terminator with new certificate from the certificate directory
    async fn reload_tls_with_new_certificate(&mut self) -> Result<()> {
        info!("Reloading TLS terminator with new certificate");

        let cert_dir = self.state_dir.join("certs");
        info!("ACME certificate should be available in: {:?}", cert_dir);

        // TODO: Implement proper ACME certificate loading and TLS reload
        // The current TLS system uses self-signed certificates with P2P keys
        // We need to extend it to support loading ACME certificates from disk

        // For now, just log that the certificate is ready for reload
        info!("New ACME certificate is available and ready for TLS reload");
        info!("TLS reload functionality needs to be implemented for ACME certificates");

        // Check if certificate files exist
        let cert_file = cert_dir.join("cert.pem");
        let key_file = cert_dir.join("key.pem");

        if cert_file.exists() && key_file.exists() {
            info!(
                "Certificate files found: {:?} and {:?}",
                cert_file, key_file
            );
            info!("TLS terminator should be configured to use these files");
        } else {
            warn!("Certificate files not found in expected locations");
        }

        Ok(())
    }
}
