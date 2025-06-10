# hellas-gate-p2p

P2P networking for Gate using [Iroh](https://iroh.computer/).

## Overview

This crate provides peer-to-peer networking capabilities for the Gate system, enabling secure communication between nodes using Iroh's QUIC-based transport with built-in hole punching and relay fallback.

## Features

- **Secure P2P connections** using Iroh's encrypted QUIC transport
- **Automatic peer discovery** via Iroh's built-in mechanisms
- **Connection management** with automatic pooling and lifecycle handling
- **Direct and relay connections** with automatic path selection
- **Custom protocol** identification ("gate/1.0")

## API

### Core Types

```rust
use hellas_gate_p2p::{P2PNode, Result};

// Create a new P2P node
let node = P2PNode::new().await?;

// Get node information
let node_id = node.node_id();
let node_addr = node.node_addr().await?;

// Start accepting connections
node.start_listening().await?;

// Connect to a peer
node.connect_to_peer(peer_addr).await?;

// Send a message
node.send_message(peer_id, b"Hello, peer!").await?;

// List connected peers
let peers = node.connected_peers().await;
```

### P2PNode

The main interface for P2P operations:

- `new()` - Create a new P2P node with random identity
- `node_id()` - Get this node's unique identifier
- `node_addr()` - Get this node's address for others to connect to
- `start_listening()` - Begin accepting incoming connections
- `connect_to_peer(addr)` - Connect to a remote peer
- `send_message(peer_id, data)` - Send raw bytes to a connected peer
- `connected_peers()` - List currently connected peer IDs
- `shutdown()` - Gracefully shutdown the node (10 second timeout)
- `shutdown_with_timeout(duration)` - Gracefully shutdown with custom timeout
- `is_shutting_down()` - Check if node is currently shutting down

## Implementation Details

### Transport

- Uses Iroh's QUIC transport for authenticated encryption
- Custom ALPN protocol identifier: "gate/1.0"
- Automatic hole punching for direct connections
- Relay fallback via Iroh's public relay network

### Connection Management

- Maintains a pool of active connections
- Automatic connection establishment and teardown
- Concurrent stream handling for multiple messages
- Connection tracking by peer NodeId

### Error Handling

- Custom `P2PError` type with automatic conversions
- Handles Iroh connection errors and general failures
- Structured error reporting for debugging

### Graceful Shutdown

- Cancellation token-based shutdown signaling
- Closes all active connections cleanly using Iroh's `close()` methods
- Waits for background tasks to complete with configurable timeout
- Prevents new operations after shutdown initiated
- Connection handler tasks respond to shutdown signals

## Testing

The crate includes comprehensive tests:

```bash
# Run all tests with tracing output
RUST_LOG=hellas_gate_p2p=debug cargo test --package hellas-gate-p2p -- --nocapture

# Run specific integration test
cargo test --package hellas-gate-p2p test_two_nodes_connect_and_communicate -- --nocapture
```

### Test Coverage

- ✅ Node creation and initialization
- ✅ Two-node connection establishment
- ✅ Bidirectional communication
- ✅ Connection tracking and peer listing
- ✅ Message delivery verification
- ✅ Graceful shutdown without connections
- ✅ Graceful shutdown with active connections
- ✅ Shutdown timeout handling
- ✅ Operations after shutdown (failure scenarios)

## Future Work

- [ ] Structured message protocol (currently raw bytes)
- [ ] Request/response correlation
- [ ] Message handlers and routing
- [ ] Trust management and peer validation
- [ ] Connection retry and failure handling
- [ ] Performance optimization and benchmarking

## Dependencies

- `iroh` - Core P2P networking and QUIC transport
- `tokio` - Async runtime
- `tracing` - Structured logging
- `serde` - Serialization (for future message protocols)
- `thiserror` - Error handling
- `anyhow` - Error compatibility with Iroh

## Development

This crate uses `test-log` for enhanced test output with tracing. The development environment automatically sets `RUST_LOG=info` for convenient debugging.
