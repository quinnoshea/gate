//! HTTP server for Gate daemon API and web interface

use crate::config::HttpConfig;
use crate::upstream::{InferenceRequest, UpstreamClient};
use crate::{DaemonError, Result};

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
    Router,
};
use hellas_gate_core::GateId;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::{ServiceBuilder, ServiceExt};
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};
use tracing::{info, warn};

/// HTTP server state shared across handlers
#[derive(Clone)]
pub struct AppState {
    upstream_client: Arc<UpstreamClient>,
    gate_id: GateId,
}

/// HTTP server for the Gate daemon
pub struct HttpServer {
    config: HttpConfig,
    app_state: AppState,
}

/// Node status response
#[derive(Debug, Serialize)]
pub struct NodeStatus {
    pub gate_id: String,
    pub status: String,
    pub uptime_secs: u64,
    pub connections: u32,
}

impl HttpServer {
    /// Create a new HTTP server
    ///
    /// # Errors
    ///
    /// Returns an error if server initialization fails
    pub fn new(
        config: HttpConfig,
        upstream_client: Arc<UpstreamClient>,
        gate_id: GateId,
    ) -> Result<Self> {
        let app_state = AppState {
            upstream_client,
            gate_id,
        };

        Ok(Self { config, app_state })
    }

    /// Start the HTTP server
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to start
    pub async fn start(&self) -> Result<()> {
        let app = self.create_app();

        let listener = TcpListener::bind(self.config.bind_addr)
            .await
            .map_err(|e| {
                DaemonError::Http(format!("Failed to bind to {}: {e}", self.config.bind_addr))
            })?;

        info!("HTTP server listening on {}", self.config.bind_addr);

        axum::serve(listener, app)
            .await
            .map_err(|e| DaemonError::Http(format!("HTTP server error: {e}")))?;

        Ok(())
    }

    /// Handle a stream connection directly (for TLS bridge connections)
    pub async fn handle_stream<S>(&self, stream: S) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        self.handle_stream_with_p2p_info(stream, None).await
    }

    /// Handle a stream connection with P2P connection information
    pub async fn handle_stream_with_p2p_info<S>(
        &self,
        stream: S,
        p2p_info: Option<crate::daemon::P2pConnectionInfo>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        let mut app = self.create_app();
        
        // Add P2P connection info as extension if provided
        if let Some(p2p_info) = p2p_info {
            info!("Handling P2P stream connection from node: {:?}", p2p_info.connection.remote_node_id());
            app = app.layer(axum::Extension(p2p_info));
        } else {
            info!("Handling direct stream connection");
        }
        
        // Wrap tokio stream for hyper compatibility
        let io = hyper_util::rt::TokioIo::new(stream);
        
        // Use hyper directly to handle the stream
        let service = hyper::service::service_fn(move |req| {
            let app = app.clone();
            async move {
                app.oneshot(req).await.map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                })
            }
        });

        if let Err(e) = hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
        {
            warn!("Stream connection error: {}", e);
            return Err(DaemonError::Http(format!("Stream connection failed: {e}")));
        }

        info!("Stream connection completed successfully");
        Ok(())
    }

    /// Shutdown the HTTP server
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown fails
    pub async fn shutdown(&mut self) -> Result<()> {
        // HTTP server shutdown is handled by dropping the server task
        info!("HTTP server shutdown complete");
        Ok(())
    }

    /// Create the Axum application with routes
    fn create_app(&self) -> Router {
        create_router_with_config(&self.app_state, &self.config)
    }
}

/// Create an Axum router with all routes and middleware
/// This function can be used by both the HTTP server and P2P TLS handler
pub fn create_router_with_config(app_state: &AppState, config: &HttpConfig) -> Router {
    let mut app = Router::new()
        // Health check
        .route("/health", get(health_check))
        // Node status
        .route("/status", get(node_status))
        // Inference API (OpenAI-compatible)
        .route("/v1/chat/completions", post(chat_completions))
        // Node management
        .route("/peers", get(list_peers))
        .route("/peers/:peer_id/connect", post(connect_peer))
        .with_state(app_state.clone());

    // Add middleware
    let service_builder = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(std::time::Duration::from_secs(
            config.timeout_secs,
        )));

    app = app.layer(service_builder);

    // Add CORS if enabled
    if config.cors_enabled {
        app = app.layer(CorsLayer::permissive());
    }

    app
}

/// Create a simple router for P2P TLS connections (without timeout/CORS middleware)
pub fn create_simple_router(app_state: &AppState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health_check))
        // Node status
        .route("/status", get(node_status))
        // Inference API (OpenAI-compatible)
        .route("/v1/chat/completions", post(chat_completions))
        // Node management
        .route("/peers", get(list_peers))
        .route("/peers/:peer_id/connect", post(connect_peer))
        .with_state(app_state.clone())
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Node status endpoint
async fn node_status(State(state): State<AppState>) -> ResponseJson<NodeStatus> {
    let status = NodeStatus {
        gate_id: state.gate_id.to_string(),
        status: "running".to_string(),
        uptime_secs: 0, // TODO: Track actual uptime
        connections: 0, // TODO: Get actual connection count
    };

    ResponseJson(status)
}

/// Chat completions endpoint (OpenAI-compatible)
async fn chat_completions(
    State(state): State<AppState>,
    Json(payload): Json<JsonValue>,
) -> impl axum::response::IntoResponse {
    // Create inference request from JSON payload
    let request = match InferenceRequest::new(payload) {
        Ok(req) => req,
        Err(e) => {
            warn!("Invalid request payload: {e}");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    info!(
        "Processing inference request for model: {}",
        request.model()
    );

    // Forward to upstream provider
    match state.upstream_client.chat_completion(request).await {
        Ok(upstream_response) => ResponseJson(upstream_response.response).into_response(),
        Err(e) => {
            warn!("Upstream inference failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// List connected peers
async fn list_peers(State(_state): State<AppState>) -> ResponseJson<Vec<String>> {
    // TODO: Get actual peer list from P2P session
    ResponseJson(vec![])
}

/// Connect to a peer
async fn connect_peer(State(_state): State<AppState>, Path(_peer_id): Path<String>) -> StatusCode {
    // TODO: Implement peer connection
    StatusCode::NOT_IMPLEMENTED
}
