//! HTTP server utilities for handling streams from various sources

use axum::Router;
#[cfg(not(target_arch = "wasm32"))]
use axum::extract::Request;
#[cfg(not(target_arch = "wasm32"))]
use hyper::body::Incoming;
#[cfg(not(target_arch = "wasm32"))]
use hyper_util::rt::{TokioExecutor, TokioIo};
#[cfg(not(target_arch = "wasm32"))]
use hyper_util::server;
use tokio::io::{AsyncRead, AsyncWrite};
#[cfg(not(target_arch = "wasm32"))]
use tower::Service;
#[cfg(not(target_arch = "wasm32"))]
use tracing::{debug, error, info};

/// HTTP server that can handle streams from various sources (TCP, P2P, etc.)
#[derive(Clone, Debug)]
pub struct HttpServer {
    router: Router<()>,
}

impl HttpServer {
    /// Create a new HTTP server with the given router
    pub fn new(router: Router<()>) -> Self {
        Self { router }
    }

    /// Handle a stream
    #[cfg(not(target_arch = "wasm32"))]
    #[tracing::instrument(name = "http.handle_stream", skip_all)]
    pub async fn handle_stream<S>(&self, stream: S) -> anyhow::Result<()>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        debug!("Handling stream");

        // Clone router
        let router = self.router.clone();

        // Wrap the stream for hyper compatibility
        let io = TokioIo::new(stream);

        // Router<()> implements Service<Request<Incoming>> directly
        let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
            router.clone().call(request)
        });

        // Serve the connection using hyper's low-level API
        // This supports both HTTP/1 and HTTP/2 with upgrades (needed for WebSockets)
        let result = server::conn::auto::Builder::new(TokioExecutor::new())
            .serve_connection_with_upgrades(io, hyper_service)
            .await;

        match result {
            Ok(()) => {
                debug!("Stream handling completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("HTTP connection error: {}", e);
                Err(anyhow::anyhow!("HTTP connection error: {}", e))
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn handle_stream<S>(&self, _stream: S) -> anyhow::Result<()>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        Err(anyhow::anyhow!("Stream handling is not supported on WASM"))
    }

    /// Handle a P2P stream
    #[cfg(not(target_arch = "wasm32"))]
    #[tracing::instrument(
        name = "http.handle_p2p_stream",
        skip_all,
        fields(
            p2p.node_id = %node_id,
            p2p.relay_domain = _relay_domain.as_deref().unwrap_or("direct")
        )
    )]
    pub async fn handle_p2p_stream<S>(
        &self,
        stream: S,
        node_id: impl std::fmt::Display,
        _relay_domain: Option<String>,
    ) -> anyhow::Result<()>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        info!("Handling P2P stream from node: {}", node_id);
        self.handle_stream(stream).await
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn handle_p2p_stream<S>(
        &self,
        _stream: S,
        _node_id: impl std::fmt::Display,
        _relay_domain: Option<String>,
    ) -> anyhow::Result<()>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        Err(anyhow::anyhow!(
            "P2P stream handling is not supported on WASM"
        ))
    }
}
