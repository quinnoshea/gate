//! P2P protocol definitions for control stream and stream coordination

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a stream within a connection
pub type StreamId = u32;

/// Node capabilities that are exchanged during handshake
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Capabilities {
    pub node_id: iroh::NodeId,
    pub protocol_version: u8,
    pub supported_stream_types: Vec<StreamType>,
    pub max_concurrent_streams: u32,
    pub supported_models: Vec<ModelInfo>,
    pub load_factor: f32, // 0.0 = idle, 1.0 = overloaded
}

/// Information about an available AI model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelInfo {
    pub name: String,
    pub provider: String, // "ollama", "lmstudio", etc.
    pub context_length: Option<u32>,
    pub capabilities: Vec<String>, // "chat", "completion", "embedding"
}

/// Types of streams that can be opened between peers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StreamType {
    /// HTTP inference requests/responses as JSON
    HttpInference,
    /// Raw TLS bytes for SNI proxy (relay functionality)
    SniProxy,
    /// File transfer (future use)
    FileTransfer,
}

/// Control stream messages for authentication and coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlMessage {
    /// Message ID for request/response correlation
    pub id: MessageId,
    /// Unix timestamp
    pub timestamp: u64,
    /// Message payload
    pub payload: ControlPayload,
}

/// UUID-like message identifier
pub type MessageId = [u8; 16];

/// Control message payload types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlPayload {
    /// Initial handshake message
    Handshake {
        node_id: iroh::NodeId,
        protocol_version: u8,
        capabilities: Capabilities,
    },
    /// Response to handshake
    HandshakeResponse {
        accepted: bool,
        reason: Option<String>,
        capabilities: Option<Capabilities>,
    },
    /// Request to open a new typed stream
    OpenStream {
        stream_id: StreamId,
        stream_type: StreamType,
        metadata: HashMap<String, String>, // Additional stream-specific metadata
    },
    /// Response to stream open request
    StreamResponse {
        stream_id: StreamId,
        accepted: bool,
        reason: Option<String>,
    },
    /// Close an existing stream
    CloseStream {
        stream_id: StreamId,
        reason: Option<String>,
    },
    /// Query peer capabilities (can be sent anytime)
    CapabilityRequest,
    /// Response with current capabilities
    CapabilityResponse { capabilities: Capabilities },
    /// Keep-alive ping
    Ping { nonce: u64 },
    /// Keep-alive pong response
    Pong { nonce: u64 },
    /// Generic error message
    Error {
        code: u32,
        message: String,
        related_message_id: Option<MessageId>,
    },
}

impl ControlMessage {
    /// Create a new control message with generated ID and current timestamp
    ///
    /// Uses a monotonic timestamp that's safe from clock adjustments.
    /// Falls back to 0 if system time is unavailable.
    #[must_use]
    pub fn new(payload: ControlPayload) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0); // Safe fallback for clock issues

        Self {
            id: Self::generate_message_id(),
            timestamp,
            payload,
        }
    }

    /// Generate a random message ID
    fn generate_message_id() -> MessageId {
        use rand::RngCore;
        let mut id = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut id);
        id
    }

    /// Create a handshake message
    #[must_use]
    pub fn handshake(node_id: iroh::NodeId, capabilities: Capabilities) -> Self {
        Self::new(ControlPayload::Handshake {
            node_id,
            protocol_version: 1,
            capabilities,
        })
    }

    /// Create a handshake response
    #[must_use]
    pub fn handshake_response(accepted: bool, capabilities: Option<Capabilities>) -> Self {
        Self::new(ControlPayload::HandshakeResponse {
            accepted,
            reason: if accepted {
                None
            } else {
                Some("Handshake rejected".to_string())
            },
            capabilities,
        })
    }

    /// Create a stream open request
    #[must_use]
    pub fn open_stream(stream_id: StreamId, stream_type: StreamType) -> Self {
        Self::new(ControlPayload::OpenStream {
            stream_id,
            stream_type,
            metadata: HashMap::new(),
        })
    }

    /// Create a stream response
    #[must_use]
    pub fn stream_response(stream_id: StreamId, accepted: bool, reason: Option<String>) -> Self {
        Self::new(ControlPayload::StreamResponse {
            stream_id,
            accepted,
            reason,
        })
    }

    /// Create a ping message
    #[must_use]
    pub fn ping(nonce: u64) -> Self {
        Self::new(ControlPayload::Ping { nonce })
    }

    /// Create a pong response
    #[must_use]
    pub fn pong(nonce: u64) -> Self {
        Self::new(ControlPayload::Pong { nonce })
    }

    /// Serialize message to JSON bytes
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails
    pub fn to_bytes(&self) -> crate::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize message from JSON bytes
    ///
    /// # Errors
    ///
    /// Returns an error if JSON deserialization fails or the data is malformed
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let node_id = iroh::NodeId::from_bytes(&[1u8; 32]).unwrap();
        let capabilities = Capabilities {
            node_id,
            protocol_version: 1,
            supported_stream_types: vec![StreamType::HttpInference],
            max_concurrent_streams: 10,
            supported_models: vec![ModelInfo {
                name: "test-model".to_string(),
                provider: "test".to_string(),
                context_length: Some(4096),
                capabilities: vec!["chat".to_string()],
            }],
            load_factor: 0.5,
        };

        let message = ControlMessage::handshake(node_id, capabilities);

        // Test serialization
        let bytes = message.to_bytes().unwrap();
        assert!(!bytes.is_empty());

        // Test deserialization
        let deserialized = ControlMessage::from_bytes(&bytes).unwrap();
        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.timestamp, deserialized.timestamp);
    }

    #[test]
    fn test_stream_types() {
        let stream_types = vec![
            StreamType::HttpInference,
            StreamType::SniProxy,
            StreamType::FileTransfer,
        ];

        for stream_type in stream_types {
            let message = ControlMessage::open_stream(1, stream_type.clone());
            let bytes = message.to_bytes().unwrap();
            let deserialized = ControlMessage::from_bytes(&bytes).unwrap();

            if let ControlPayload::OpenStream {
                stream_type: deserialized_type,
                ..
            } = deserialized.payload
            {
                assert_eq!(stream_type, deserialized_type);
            } else {
                panic!("Expected OpenStream payload");
            }
        }
    }

    #[test]
    fn test_ping_pong() {
        let nonce = 12345u64;

        let ping_msg = ControlMessage::ping(nonce);
        let pong_msg = ControlMessage::pong(nonce);

        let ping_serialized = ping_msg.to_bytes().unwrap();
        let pong_serialized = pong_msg.to_bytes().unwrap();

        let ping_deserialized = ControlMessage::from_bytes(&ping_serialized).unwrap();
        let pong_deserialized = ControlMessage::from_bytes(&pong_serialized).unwrap();

        if let ControlPayload::Ping { nonce: ping_nonce } = ping_deserialized.payload {
            assert_eq!(nonce, ping_nonce);
        } else {
            panic!("Expected Ping payload");
        }

        if let ControlPayload::Pong { nonce: pong_nonce } = pong_deserialized.payload {
            assert_eq!(nonce, pong_nonce);
        } else {
            panic!("Expected Pong payload");
        }
    }
}
