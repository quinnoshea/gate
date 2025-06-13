//! Bridge between P2P TLS streams and Axum HTTP handler via channels

use crate::upstream::UpstreamClient;
use crate::{DaemonError, Result};
use hellas_gate_core::GateId;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Bridge that connects decrypted byte streams directly to Axum
pub struct TlsBridge {
    upstream_client: Arc<UpstreamClient>,
    gate_id: GateId,
}

impl TlsBridge {
    /// Create a new TLS bridge
    pub fn new(upstream_client: Arc<UpstreamClient>, gate_id: GateId) -> Result<Self> {
        Ok(Self {
            upstream_client,
            gate_id,
        })
    }

    /// Create a bidirectional stream pair for connecting TLS decrypter to Axum
    /// Returns (client_stream, server_stream) where:
    /// - client_stream: for the TLS decrypter to read/write decrypted bytes
    /// - server_stream: for Axum to handle as a normal TCP-like connection
    pub fn create_stream_pair(&self) -> (TlsClientStream, TlsServerStream) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();

        let client_stream = TlsClientStream {
            tx: client_tx,
            rx: client_rx,
        };

        let server_stream = TlsServerStream {
            tx: server_tx,
            rx: server_rx,
            upstream_client: self.upstream_client.clone(),
            gate_id: self.gate_id,
        };

        (client_stream, server_stream)
    }
}

/// Client side of the stream pair (for TLS decrypter)
pub struct TlsClientStream {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl TlsClientStream {
    /// Send decrypted bytes to Axum
    pub async fn send(&mut self, data: Vec<u8>) -> Result<()> {
        self.tx.send(data).map_err(|_| DaemonError::Http("Stream closed".to_string()))?;
        Ok(())
    }

    /// Receive response bytes from Axum
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.rx.recv().await
    }
}

/// Server side of the stream pair (for Axum)
pub struct TlsServerStream {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    rx: mpsc::UnboundedReceiver<Vec<u8>>,
    upstream_client: Arc<UpstreamClient>,
    gate_id: GateId,
}

impl AsyncRead for TlsServerStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.rx.poll_recv(cx) {
            std::task::Poll::Ready(Some(data)) => {
                let to_copy = std::cmp::min(data.len(), buf.remaining());
                buf.put_slice(&data[..to_copy]);
                debug!("TLS bridge read {} bytes", to_copy);
                std::task::Poll::Ready(Ok(()))
            }
            std::task::Poll::Ready(None) => {
                debug!("TLS bridge stream closed");
                std::task::Poll::Ready(Ok(()))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl AsyncWrite for TlsServerStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        match self.tx.send(buf.to_vec()) {
            Ok(()) => {
                debug!("TLS bridge wrote {} bytes", buf.len());
                std::task::Poll::Ready(Ok(buf.len()))
            }
            Err(_) => {
                warn!("TLS bridge write failed - stream closed");
                std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Stream closed",
                )))
            }
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stream_pair_creation() {
        let upstream_client = Arc::new(crate::upstream::UpstreamClient::new(
            &crate::config::UpstreamConfig::default(),
        ).unwrap());
        let gate_id = GateId::from_bytes([0u8; 32]);
        
        let bridge = TlsBridge::new(upstream_client, gate_id).unwrap();
        let (_client_stream, _server_stream) = bridge.create_stream_pair();
        
        // Test that streams can be created successfully
        assert!(true);
    }
}