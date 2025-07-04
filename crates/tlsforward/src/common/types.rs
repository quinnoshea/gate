//! Common types used by both client and server

use iroh::NodeId;
use serde::{Deserialize, Serialize};

/// API Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code for programmatic handling
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Optional additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Information about a TLS forward endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsForwardInfo {
    /// TLS forward node ID
    pub node_id: NodeId,
    // /// TLS forward node address
    // pub node_addr: NodeAddr,
    /// Domain suffix for TLS forward addresses
    pub domain_suffix: String,
}

/// Registration request from a gate server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationRequest {}

/// Registration response from TLS forward server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResponse {
    /// Assigned domain (e.g., "abc123.private.hellas.ai")
    pub domain: String,
    /// TLS forward information
    pub tlsforward_info: TlsForwardInfo,
}

/// Certificate provisioning status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertificateStatus {
    /// No certificate needed yet
    NotRequired,
    /// Certificate is being provisioned
    Provisioning,
    /// Certificate is ready
    Ready {
        /// Certificate expiry timestamp
        expires_at: u64,
    },
    /// Certificate provisioning failed
    Failed {
        /// Error message
        error: String,
    },
}

// DNS Challenge Management Types

/// Request to create a DNS challenge
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateChallengeRequest {
    /// The domain for the challenge (e.g., "abc123.private.hellas.ai")
    pub domain: String,
    /// The challenge subdomain (typically "_acme-challenge")
    pub challenge: String,
    /// The challenge value to set in the TXT record
    pub value: String,
}

/// Response for challenge creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChallengeResponse {
    /// Unique ID for this challenge
    pub id: String,
    /// Current status of the challenge
    pub status: ChallengeStatus,
}

/// Status of a DNS challenge
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeStatus {
    /// Challenge created but not yet propagated
    Pending,
    /// Challenge has propagated to DNS
    Propagated,
    /// Challenge failed to create or propagate
    Failed { error: String },
}

/// Response for challenge status check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeStatusResponse {
    /// Challenge ID
    pub id: String,
    /// Current status
    pub status: ChallengeStatus,
    /// Number of successful DNS checks (if propagating)
    pub checks: u32,
}

/// Response for challenge deletion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteChallengeResponse {
    /// Challenge ID
    pub id: String,
    /// Status after deletion
    pub status: String,
}

/// Status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    /// Whether the node is registered
    pub registered: bool,
    /// Assigned domain if registered
    pub domain: Option<String>,
    /// Number of active connections
    pub active_connections: usize,
    /// Uptime in seconds
    pub uptime_seconds: u64,
}

/// Empty response type for operations that don't return data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyResponse {}

/// Information about a connected node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedNode {
    /// Node ID
    pub node_id: String,
    /// Assigned domain
    pub domain: String,
    /// Connection timestamp (ISO 8601)
    pub connected_at: String,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Latency in milliseconds (if measured)
    pub latency_ms: Option<u64>,
    /// Last ping timestamp (ISO 8601)
    pub last_ping: String,
}

/// Response for listing connected nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNodesResponse {
    /// List of connected nodes
    pub nodes: Vec<ConnectedNode>,
    /// Total number of connected nodes
    pub total: usize,
}
