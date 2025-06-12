//! Request and response abstractions for P2P protocols

use crate::{P2PError, P2PStream, Result};
use hellas_gate_core::GateId;
use iroh::{endpoint::RecvStream, NodeId};
use serde_json::Value as JsonValue;
use tokio::io::{AsyncRead, AsyncWrite};

/// Convert iroh `NodeId` to `GateId`
fn node_id_to_gate_id(node_id: NodeId) -> GateId {
    GateId::from_bytes(*node_id.as_bytes())
}

/// SNI proxy request for raw TCP-like forwarding
pub struct SniProxyRequest {
    peer_id: NodeId,
    domain: String,
    stream: P2PStream,
}

impl SniProxyRequest {
    /// Create a new SNI proxy request
    pub fn new(peer_id: NodeId, domain: String, stream: P2PStream) -> Self {
        Self {
            peer_id,
            domain,
            stream,
        }
    }

    /// Get the peer ID that sent this request
    pub fn peer_id(&self) -> GateId {
        node_id_to_gate_id(self.peer_id)
    }

    /// Get the domain being requested
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Forward raw bytes to peer
    pub async fn forward_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.stream.send_bytes(data).await
    }

    /// Read raw bytes from peer
    pub async fn read_bytes(&mut self) -> Result<Vec<u8>> {
        self.stream.recv_bytes().await
    }

    /// Split into read/write halves for bidirectional forwarding
    pub fn split(self) -> (SniProxyReader, SniProxyWriter) {
        let (send, recv) = self.stream.into_split();
        (
            SniProxyReader { recv },
            SniProxyWriter {
                send,
                peer_id: self.peer_id,
            },
        )
    }
}

/// Reader half of SNI proxy connection
pub struct SniProxyReader {
    pub recv: RecvStream,
}

impl SniProxyReader {
    /// Read bytes from the peer
    pub async fn read_bytes(&mut self) -> Result<Vec<u8>> {
        // Read a chunk of data from the stream
        let chunk =
            self.recv.read_chunk(8192, false).await.map_err(|e| {
                P2PError::ConnectionFailed(format!("Failed to read from stream: {e}"))
            })?;

        match chunk {
            Some(bytes) => Ok(bytes.bytes.to_vec()),
            None => Err(P2PError::ConnectionFailed("Stream closed".to_string())),
        }
    }
}

impl AsyncRead for SniProxyReader {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

/// Writer half of SNI proxy connection
pub struct SniProxyWriter {
    pub send: iroh::endpoint::SendStream,
    pub peer_id: NodeId,
}

impl SniProxyWriter {
    /// Write bytes to the peer
    pub async fn write_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.send
            .write_all(data)
            .await
            .map_err(|e| P2PError::ConnectionFailed(format!("Failed to write to stream: {e}")))?;
        Ok(())
    }
}

impl AsyncWrite for SniProxyWriter {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        std::pin::Pin::new(&mut self.send)
            .poll_write(cx, buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.send).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.send).poll_shutdown(cx)
    }
}

/// Inference request with streaming response support
pub struct InferenceRequest {
    pub peer_id: GateId,
    pub request_data: JsonValue,
    pub stream: P2PStream,
}

impl InferenceRequest {
    /// Create a new inference request
    pub fn new(peer_id: NodeId, request_data: JsonValue, stream: P2PStream) -> Self {
        Self {
            peer_id: node_id_to_gate_id(peer_id),
            request_data,
            stream,
        }
    }

    /// Send a JSON response
    pub async fn send_json(&mut self, response: &JsonValue) -> Result<()> {
        self.stream.send_json(response).await
    }
}

impl AsyncWrite for InferenceRequest {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        std::pin::Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
