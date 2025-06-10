# hellas-gate-p2p

P2P networking for Gate using [Iroh](https://iroh.computer/) with multi-stream protocol support.

## Overview

This crate provides a complete peer-to-peer networking system for the Gate network, featuring a multi-stream protocol architecture that supports different types of communication over a single connection. Built on Iroh's secure QUIC transport with automatic hole punching and relay fallback.

## Architecture

### Multi-Stream Protocol

Gate uses a sophisticated multi-stream protocol over each P2P connection:

- **Control Stream (ID 0)**: JSON messages for handshake, authentication, and stream coordination
- **Inference Streams**: JSON request/response envelopes for AI operations (chat completions, model queries)
- **SNI Proxy Streams**: Raw TLS bytes for transparent HTTPS forwarding (relay functionality)

### High-Level API

The `P2PNode` provides a clean API that hides all stream management complexity:

```rust
use hellas_gate_p2p::{P2PNode, ChatCompletionRequest, Result};

// Create and connect
let node = P2PNode::new().await?;
node.connect_to_peer(peer_addr).await?;

// High-level inference operations
let response = node.send_chat_completion(peer_id, request).await?;
let models = node.list_peer_models(peer_id).await?;

// SNI proxy for relay functionality
let proxy_handle = node.open_sni_proxy(peer_id, "example.com".to_string()).await?;

// Capability management
node.update_capabilities(capabilities).await;
let peer_caps = node.get_peer_capabilities(peer_id).await;
```

## API Reference

### P2PNode

**Connection Management:**
- `new()` - Create a new P2P node with auto-generated capabilities
- `node_id()` - Get this node's unique identifier (Ed25519 public key)
- `node_addr()` - Get this node's address for others to connect to
- `connect_to_peer(addr)` - Connect to a remote peer with stream management
- `connected_peers()` - List currently connected peer IDs
- `shutdown()` / `shutdown_with_timeout()` - Graceful shutdown with stream cleanup

**High-Level Inference API:**
- `send_chat_completion(peer_id, request)` - Send chat completion request to peer
- `list_peer_models(peer_id)` - Get available models from peer
- `get_peer_capabilities(peer_id)` - Get peer's current capabilities

**SNI Proxy API:**
- `open_sni_proxy(peer_id, domain)` - Open SNI proxy stream for domain

**Capabilities Management:**
- `update_capabilities(capabilities)` - Update local capabilities (called by daemon)

### Protocol Types

**Control Messages:**
```rust
use hellas_gate_p2p::{ControlMessage, ControlPayload, Capabilities};

// Handshake, stream requests, ping/pong, errors
let message = ControlMessage::handshake(node_id, capabilities);
let stream_request = ControlMessage::open_stream(stream_id, StreamType::HttpInference);
```

**Inference Protocol:**
```rust
use hellas_gate_p2p::{InferenceRequest, InferenceResponse, ChatCompletionRequest};

// OpenAI-compatible requests with correlation IDs
let request = InferenceRequest::chat_completion(request_id, chat_request);
let response = InferenceResponse::chat_completion(request_id, chat_response);
```

**SNI Proxy:**
```rust
use hellas_gate_p2p::{SniProxyStream, SniProxyConfig};

// Transparent TLS byte forwarding
let proxy = SniProxyStream::new(stream_id, peer_id);
proxy.configure(SniProxyConfig { domain: "example.com".to_string(), .. });
proxy.handle_stream(stream1, stream2).await?; // Bidirectional copy
```

## Protocol Flow

1. **Connection Establishment**: Iroh QUIC connection with "gate/1.0" ALPN
2. **Control Stream Setup**: Stream ID 0 opened automatically for coordination
3. **Handshake Exchange**: Capabilities and trust verification
4. **Typed Stream Requests**: Open inference/SNI proxy streams via control messages
5. **Data Exchange**: Protocol-specific communication on each stream
6. **Keep-Alive**: Ping/pong on control stream maintains connection health

## Implementation Status

**✅ Completed:**
- Multi-stream protocol design and message types
- P2PNode with clean high-level API
- Control, inference, and SNI proxy protocol definitions
- Connection management with per-peer state
- Stream lifecycle types and coordination messages
- Comprehensive protocol message serialization
- Integration-ready API for HTTP server

**🔄 Still TODO:**
- Actual bidirectional stream communication implementation
- Control stream handshake and message exchange
- Request/response correlation across streams
- Stream opening/closing coordination
- Error handling and timeout management

## Testing

```bash
# Run all tests
cargo test --package hellas-gate-p2p

# Run with debug logging
RUST_LOG=hellas_gate_p2p=debug cargo test --package hellas-gate-p2p -- --nocapture

# Test specific protocols
cargo test --package hellas-gate-p2p inference::tests
cargo test --package hellas-gate-p2p sni_proxy::tests
cargo test --package hellas-gate-p2p protocol::tests
```

### Test Coverage

**✅ Protocol Types:**
- Control message serialization/deserialization
- Inference request/response structures
- SNI proxy configuration and statistics
- Stream type negotiation

**✅ P2P Foundation:**
- Node creation and connection establishment
- Connection tracking and peer management
- Graceful shutdown with cleanup
- Connection lifecycle and error scenarios

**🔄 Integration Tests Needed:**
- End-to-end multi-stream communication
- Control stream handshake flows
- Request correlation and timeout handling

## Usage in Gate

The P2P crate integrates with other Gate components:

**HTTP Server Integration:**
```rust
// HTTP server uses high-level P2P API
let response = p2p_node.send_chat_completion(peer_id, request).await?;
```

**Provider Integration:**
```rust
// Update P2P capabilities when local providers change
p2p_node.update_capabilities(updated_capabilities).await;
```

**Relay Integration:**
```rust
// Open SNI proxy for public HTTPS endpoints
let proxy = p2p_node.open_sni_proxy(peer_id, domain).await?;
```

## Dependencies

- `iroh` - Core P2P networking and QUIC transport
- `tokio` - Async runtime and channels
- `serde` / `serde_json` - Protocol message serialization
- `tracing` - Structured logging
- `thiserror` - Error handling
- `rand` - Message ID generation
- `hex` - Request ID encoding

## Security

- **Transport Encryption**: All communication encrypted via Iroh QUIC
- **Node Identity**: Ed25519 keypairs for node identification
- **Trust-Based Access**: Only configured trusted peers can make requests
- **Stream Isolation**: Different protocols isolated on separate streams
- **No Credential Transmission**: Private keys never leave local node
