//! P2P networking for Gate using Iroh

pub mod error;
pub mod inference;
pub mod node;
pub mod protocol;
pub mod sni_proxy;

pub use error::P2PError;
pub use inference::{
    ChatCompletionRequest, ChatCompletionResponse, InferenceRequest, InferenceResponse,
};
pub use node::{P2PNode, SniProxyHandle};
pub use protocol::{Capabilities, ControlMessage, ControlPayload, ModelInfo, StreamId, StreamType};
pub use sni_proxy::{SniProxyConfig, SniProxyStats, SniProxyStream};

pub type Result<T> = std::result::Result<T, P2PError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test_log::test(tokio::test)]
    async fn test_node_creation() {
        let node = P2PNode::new().await.unwrap();
        tracing::info!("Created node with ID: {}", node.node_id());
    }

    #[test_log::test(tokio::test)]
    async fn test_two_nodes_connect_and_communicate() {
        // Create two nodes
        let node1 = P2PNode::new().await.unwrap();
        let node2 = P2PNode::new().await.unwrap();

        tracing::info!("Created node1 with ID: {}", node1.node_id());
        tracing::info!("Created node2 with ID: {}", node2.node_id());

        // Node1 is already listening automatically

        // Give it a moment to start listening
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Get node1's address
        let node1_addr = node1.node_addr().await.unwrap();
        tracing::info!("Node1 address: {:?}", node1_addr);

        // Connect node2 to node1
        node2.connect_to_peer(node1_addr).await.unwrap();
        tracing::info!("Node2 connected to node1");

        // Give connection time to establish
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify they're connected
        let connected_peers_node2 = node2.connected_peers().await;
        let connected_peers_node1 = node1.connected_peers().await;

        tracing::info!("Node2 connected peers: {:?}", connected_peers_node2);
        tracing::info!("Node1 connected peers: {:?}", connected_peers_node1);

        assert!(
            connected_peers_node2.contains(&node1.node_id()),
            "Node2 should be connected to node1"
        );
        assert!(
            connected_peers_node1.contains(&node2.node_id()),
            "Node1 should be connected to node2"
        );

        // Send a message from node2 to node1
        let test_message = b"Hello from node2!";
        node2
            .send_message(node1.node_id(), test_message)
            .await
            .unwrap();
        tracing::info!("Sent message from node2 to node1");

        // Give time for message to be received and logged
        tokio::time::sleep(Duration::from_millis(100)).await;

        // For now, we just verify the message was sent without error
        // In the future, we could add a mechanism to track received messages
    }

    #[test_log::test(tokio::test)]
    async fn test_graceful_shutdown() {
        let mut node = P2PNode::new().await.unwrap();
        tracing::info!("Created node with ID: {}", node.node_id());

        // Verify node is not shutting down initially
        assert!(!node.is_shutting_down());

        // Shutdown the node
        node.shutdown().await.unwrap();
        tracing::info!("Node shutdown completed");

        // Verify node is marked as shutting down
        assert!(node.is_shutting_down());
    }

    #[test_log::test(tokio::test)]
    async fn test_shutdown_with_connections() {
        // Create two nodes
        let mut node1 = P2PNode::new().await.unwrap();
        let mut node2 = P2PNode::new().await.unwrap();

        tracing::info!("Created node1 with ID: {}", node1.node_id());
        tracing::info!("Created node2 with ID: {}", node2.node_id());

        // Give nodes time to start listening
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Get node1's address and connect node2 to node1
        let node1_addr = node1.node_addr().await.unwrap();
        node2.connect_to_peer(node1_addr).await.unwrap();
        tracing::info!("Node2 connected to node1");

        // Give connection time to establish
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify they're connected
        let connected_peers_node1 = node1.connected_peers().await;
        let connected_peers_node2 = node2.connected_peers().await;

        assert!(connected_peers_node1.contains(&node2.node_id()));
        assert!(connected_peers_node2.contains(&node1.node_id()));

        // Shutdown node1 (which has incoming connection)
        tracing::info!("Shutting down node1");
        node1.shutdown().await.unwrap();

        // Shutdown node2
        tracing::info!("Shutting down node2");
        node2.shutdown().await.unwrap();

        tracing::info!("Both nodes shutdown successfully");
    }

    #[test_log::test(tokio::test)]
    async fn test_shutdown_timeout() {
        let mut node = P2PNode::new().await.unwrap();
        tracing::info!("Created node with ID: {}", node.node_id());

        // Shutdown with very short timeout should still succeed
        // since there are no connections
        node.shutdown_with_timeout(Duration::from_millis(100))
            .await
            .unwrap();
        tracing::info!("Node shutdown with timeout completed");

        assert!(node.is_shutting_down());
    }

    #[test_log::test(tokio::test)]
    async fn test_operations_after_shutdown() {
        let mut node = P2PNode::new().await.unwrap();

        // Shutdown the node
        node.shutdown().await.unwrap();

        // Try to perform operations after shutdown - they should fail gracefully
        // Create a dummy node address (this will fail to connect since it's not a real node)
        let dummy_node_id = iroh::NodeId::from_bytes(&[0u8; 32]).unwrap();
        let dummy_addr = iroh::NodeAddr::new(dummy_node_id);
        let result = node.connect_to_peer(dummy_addr).await;

        // The exact error type may vary, but it should fail
        assert!(result.is_err());
        tracing::info!("Operations after shutdown correctly failed: {:?}", result);
    }
}
