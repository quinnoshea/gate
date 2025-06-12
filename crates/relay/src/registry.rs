use crate::error::{RelayError, Result};
use hellas_gate_core::{GateAddr, GateId};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Registry of active Gate nodes and their associated domains
pub struct NodeRegistry {
    /// Map of node ID to node information
    nodes: RwLock<HashMap<GateId, NodeInfo>>,

    /// Map of domain to node ID for fast lookup
    domains: RwLock<HashMap<String, GateId>>,

    /// Configuration for cleanup behavior
    cleanup_interval: Duration,
    node_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Node's unique identifier
    pub node_id: GateId,

    /// Full node address (ID + network address)
    pub node_addr: GateAddr,

    /// Associated domain names (can have multiple)
    pub domains: Vec<String>,

    /// P2P connection information
    pub connection_info: Option<ConnectionInfo>,

    /// When this node was last seen
    pub last_seen: SystemTime,

    /// Current status
    pub status: NodeStatus,
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Whether we have an active P2P connection
    pub connected: bool,

    /// Connection quality metrics
    pub latency: Option<Duration>,

    /// Number of active proxy connections
    pub active_connections: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    /// Node is online and accepting connections
    Online,

    /// Node is degraded (high latency, partial failures)
    Degraded,

    /// Node is offline or unreachable
    Offline,

    /// Node registration is pending (waiting for DNS propagation)
    Pending,
}

impl NodeRegistry {
    /// Create a new node registry
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            domains: RwLock::new(HashMap::new()),
            cleanup_interval: Duration::from_secs(60), // Clean up every minute
            node_timeout: Duration::from_secs(300),    // 5 minute timeout
        }
    }

    /// Register a new node with its domain
    pub async fn register_node(&self, node_addr: GateAddr, domain: String) -> Result<()> {
        let now = SystemTime::now();
        let node_id = node_addr.id;

        let node_info = NodeInfo {
            node_id,
            node_addr,
            domains: vec![domain.clone()],
            connection_info: None,
            last_seen: now,
            status: NodeStatus::Pending,
        };

        // Update both maps
        {
            let mut nodes = self.nodes.write().await;
            let mut domains = self.domains.write().await;

            nodes.insert(node_id, node_info);
            domains.insert(domain.clone(), node_id);
        }

        info!(
            "Registered node {} with domain {}",
            hex::encode(node_id.as_bytes()),
            domain
        );

        Ok(())
    }

    /// Update node status and connection info
    pub async fn update_node_status(
        &self,
        node_id: GateId,
        status: NodeStatus,
        connection_info: Option<ConnectionInfo>,
    ) -> Result<()> {
        let now = SystemTime::now();

        let mut nodes = self.nodes.write().await;

        if let Some(node_info) = nodes.get_mut(&node_id) {
            node_info.status = status;
            node_info.connection_info = connection_info;
            node_info.last_seen = now;

            debug!(
                "Updated node {} status to {:?}",
                hex::encode(node_id.as_bytes()),
                node_info.status
            );
        } else {
            warn!(
                "Attempted to update unknown node {}",
                hex::encode(node_id.as_bytes())
            );
        }

        Ok(())
    }

    /// Get node information by node ID
    pub async fn get_node(&self, node_id: &GateId) -> Option<NodeInfo> {
        let nodes = self.nodes.read().await;
        nodes.get(node_id).cloned()
    }

    /// Get node ID by domain
    pub async fn get_node_by_domain(&self, domain: &str) -> Option<GateId> {
        let domains = self.domains.read().await;
        domains.get(domain).copied()
    }

    /// Remove a node and all its domains
    pub async fn remove_node(&self, node_id: GateId) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        let mut domains = self.domains.write().await;

        if let Some(node_info) = nodes.remove(&node_id) {
            // Remove all associated domains
            for domain in &node_info.domains {
                domains.remove(domain);
            }

            info!(
                "Removed node {} and {} domains",
                hex::encode(node_id.as_bytes()),
                node_info.domains.len()
            );
        }

        Ok(())
    }

    /// Add an additional domain to an existing node
    pub async fn add_domain(&self, node_id: GateId, domain: String) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        let mut domains = self.domains.write().await;

        if let Some(node_info) = nodes.get_mut(&node_id) {
            if !node_info.domains.contains(&domain) {
                node_info.domains.push(domain.clone());
                domains.insert(domain.clone(), node_id);

                info!(
                    "Added domain {} to node {}",
                    domain,
                    hex::encode(node_id.as_bytes())
                );
            }
        } else {
            return Err(RelayError::NodeNotFound {
                node_id: hex::encode(node_id.as_bytes()),
            });
        }

        Ok(())
    }

    /// Get all registered nodes
    pub async fn list_nodes(&self) -> Vec<NodeInfo> {
        let nodes = self.nodes.read().await;
        nodes.values().cloned().collect()
    }

    /// Get statistics about the registry
    pub async fn get_stats(&self) -> RegistryStats {
        let nodes = self.nodes.read().await;
        let domains = self.domains.read().await;

        let mut online_count = 0;
        let mut degraded_count = 0;
        let mut offline_count = 0;
        let mut pending_count = 0;

        for node in nodes.values() {
            match node.status {
                NodeStatus::Online => online_count += 1,
                NodeStatus::Degraded => degraded_count += 1,
                NodeStatus::Offline => offline_count += 1,
                NodeStatus::Pending => pending_count += 1,
            }
        }

        RegistryStats {
            total_nodes: nodes.len(),
            total_domains: domains.len(),
            online_nodes: online_count,
            degraded_nodes: degraded_count,
            offline_nodes: offline_count,
            pending_nodes: pending_count,
        }
    }

    /// Start background cleanup task
    pub async fn start_cleanup_task(&self) {
        let mut interval = tokio::time::interval(self.cleanup_interval);

        loop {
            interval.tick().await;
            self.cleanup_expired_nodes().await;
        }
    }

    /// Remove nodes that haven't been seen recently
    async fn cleanup_expired_nodes(&self) {
        let now = SystemTime::now();
        let timeout = self.node_timeout;

        let mut nodes_to_remove = Vec::new();

        {
            let nodes = self.nodes.read().await;

            for (node_id, node_info) in nodes.iter() {
                if let Ok(duration) = now.duration_since(node_info.last_seen) {
                    if duration > timeout && node_info.status == NodeStatus::Offline {
                        nodes_to_remove.push(*node_id);
                    }
                }
            }
        }

        for node_id in nodes_to_remove {
            if let Err(e) = self.remove_node(node_id).await {
                warn!(
                    "Failed to remove expired node {}: {}",
                    hex::encode(node_id.as_bytes()),
                    e
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub total_nodes: usize,
    pub total_domains: usize,
    pub online_nodes: usize,
    pub degraded_nodes: usize,
    pub offline_nodes: usize,
    pub pending_nodes: usize,
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hellas_gate_core::{GateAddr, GateId};

    #[tokio::test]
    async fn test_register_and_get_node() {
        let registry = NodeRegistry::new();
        let node_id = GateId::from_bytes([1u8; 32]);
        let node_addr = GateAddr::new(node_id, "127.0.0.1:41145".to_string());
        let domain = "test.private.hellas.ai".to_string();

        registry
            .register_node(node_addr, domain.clone())
            .await
            .unwrap();

        let node_info = registry.get_node(&node_id).await.unwrap();
        assert_eq!(node_info.node_id, node_id);
        assert_eq!(node_info.domains[0], domain);
        assert_eq!(node_info.status, NodeStatus::Pending);

        let found_node_id = registry.get_node_by_domain(&domain).await.unwrap();
        assert_eq!(found_node_id, node_id);
    }

    #[tokio::test]
    async fn test_update_node_status() {
        let registry = NodeRegistry::new();
        let node_id = GateId::from_bytes([2u8; 32]);
        let node_addr = GateAddr::new(node_id, "127.0.0.1:41146".to_string());
        let domain = "test2.private.hellas.ai".to_string();

        registry.register_node(node_addr, domain).await.unwrap();

        let connection_info = ConnectionInfo {
            connected: true,
            latency: Some(Duration::from_millis(50)),
            active_connections: 3,
        };

        registry
            .update_node_status(node_id, NodeStatus::Online, Some(connection_info.clone()))
            .await
            .unwrap();

        let node_info = registry.get_node(&node_id).await.unwrap();
        assert_eq!(node_info.status, NodeStatus::Online);
        assert!(node_info.connection_info.is_some());
        assert_eq!(node_info.connection_info.unwrap().active_connections, 3);
    }

    #[tokio::test]
    async fn test_registry_stats() {
        let registry = NodeRegistry::new();

        // Add some test nodes
        let node1 = GateId::from_bytes([1u8; 32]);
        let node2 = GateId::from_bytes([2u8; 32]);
        let node1_addr = GateAddr::new(node1, "127.0.0.1:41147".to_string());
        let node2_addr = GateAddr::new(node2, "127.0.0.1:41148".to_string());

        registry
            .register_node(node1_addr, "node1.private.hellas.ai".to_string())
            .await
            .unwrap();
        registry
            .register_node(node2_addr, "node2.private.hellas.ai".to_string())
            .await
            .unwrap();

        registry
            .update_node_status(node1, NodeStatus::Online, None)
            .await
            .unwrap();
        registry
            .update_node_status(node2, NodeStatus::Degraded, None)
            .await
            .unwrap();

        let stats = registry.get_stats().await;
        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.total_domains, 2);
        assert_eq!(stats.online_nodes, 1);
        assert_eq!(stats.degraded_nodes, 1);
        assert_eq!(stats.offline_nodes, 0);
        assert_eq!(stats.pending_nodes, 0);
    }
}
