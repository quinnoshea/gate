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
use hellas_gate_p2p::GateId;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
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
            .with_state(self.app_state.clone());

        // Add middleware
        let service_builder = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(TimeoutLayer::new(std::time::Duration::from_secs(
                self.config.timeout_secs,
            )));

        app = app.layer(service_builder);

        // Add CORS if enabled
        if self.config.cors_enabled {
            app = app.layer(CorsLayer::permissive());
        }

        app
    }
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
