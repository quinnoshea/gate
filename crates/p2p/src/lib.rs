//! P2P networking for Gate using Iroh

pub mod error;
pub mod protocols;
pub mod request;
pub mod session;
pub mod stream;

pub use error::P2PError;
pub use protocols::{
    CONTROL_PROTOCOL, DOMAIN_REGISTRATION_PROTOCOL, INFERENCE_PROTOCOL, SNI_PROXY_PROTOCOL,
};
pub use request::{InferenceRequest, SniProxyRequest};
pub use session::{
    ControlMessage, DnsChallengeHandler, P2PSession, P2PSessionBuilder, PeerConnectionHandle,
};
pub use stream::P2PStream;

// Re-export core types
pub use hellas_gate_core::{GateAddr, GateId};

pub type Result<T> = std::result::Result<T, P2PError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn test_session_creation() {
        let session = P2PSession::builder().build().await.unwrap();
        let node_id = session.node_id();

        // Basic sanity check
        assert_ne!(node_id.to_string(), "");
    }

    #[test_log::test(tokio::test)]
    async fn test_handle_creation() {
        let mut session = P2PSession::builder()
            .with_inference()
            .with_sni_proxy()
            .build()
            .await
            .unwrap();

        // Should be able to take handles
        let _sni_handle = session.take_sni_proxy_handle();
        let _inference_handle = session.take_inference_handle();
    }

    #[test_log::test(tokio::test)]
    async fn test_peer_connection_and_handshake() {
        use tokio::time::{timeout, Duration};

        // Create relay session (supports SNI proxy)
        let mut relay_session = P2PSession::builder()
            .with_generated_identity()
            .with_sni_proxy()
            .build()
            .await
            .unwrap();

        // Create daemon session (supports inference)
        let mut daemon_session = P2PSession::builder()
            .with_generated_identity()
            .with_inference()
            .build()
            .await
            .unwrap();

        let relay_addr = relay_session.node_addr().await.unwrap();
        let _sni_handle = relay_session.take_sni_proxy_handle().unwrap();
        let _inference_handle = daemon_session.take_inference_handle().unwrap();

        // Daemon connects to relay as bootstrap peer
        let connection_handle = daemon_session.add_peer(relay_addr.clone()).await.unwrap();

        // Wait for connection establishment (should succeed)
        let result = timeout(Duration::from_secs(5), connection_handle.wait_connected()).await;
        assert!(result.is_ok(), "Connection should succeed within 5 seconds");

        let connected_peer_id = result.unwrap().unwrap();
        assert_eq!(connected_peer_id, relay_addr.id);

        // Give some time for capability negotiation to happen
        tokio::time::sleep(Duration::from_millis(500)).await;

        // TODO: In a full test, we would verify that:
        // 1. Handshake messages were exchanged
        // 2. Capabilities were negotiated
        // 3. Idle SNI streams were opened by the daemon
        // For now, we just verify the connection was established
    }
}
