# hellas-gate-p2p

P2P networking for Gate using [Iroh](https://iroh.computer/) with control protocol and multiple stream types.

## Overview

This crate provides peer-to-peer networking for the Gate network with support for:
- Control protocol for handshakes and coordination
- Inference streams for AI operations
- SNI proxy streams for HTTPS forwarding
- DNS challenge coordination for ACME

## Current Status

**✅ Implemented:**
- `P2PSession` with endpoint management and peer actors
- Control protocol with capability negotiation
- Inference and SNI proxy protocol support
- DNS challenge request/response messages
- Stream management and bidirectional communication

**🔄 In Progress:**
- External SNI stream management (moved to applications)
- Response correlation and timeout handling

## Architecture

### Core Components

- **P2PSession**: Main entry point, manages multiple peer connections
- **PeerActor**: Handles individual peer connections with control streams
- **Control Protocol**: JSON messages for handshakes, capabilities, and coordination
- **Stream Types**: Different protocols (inference, SNI proxy) over separate streams

### Session Builder

```rust
use hellas_gate_p2p::P2PSession;

// Daemon session (handles inference)
let session = P2PSession::builder()
    .with_port(31145)
    .with_inference()
    .with_private_key(&identity)
    .build().await?;

// Relay session (handles SNI proxy and DNS challenges)
let session = P2PSession::builder()
    .with_port(31145)
    .with_sni_proxy()
    .with_dns_challenge()
    .with_dns_challenge_handler(cloudflare_handler)
    .build().await?;
```

### Control Protocol

Messages exchanged over control streams:
- `Handshake` / `HandshakeResponse` - Capability negotiation
- `DnsChallengeCreate` / `DnsChallengeCleanup` - ACME DNS challenges
- `DnsChallengeResponse` - Challenge operation results
- `Ping` / `Pong` - Keep-alive

### DNS Challenge Integration

The P2P crate provides the `DnsChallengeHandler` trait for external DNS providers:

```rust
use hellas_gate_p2p::DnsChallengeHandler;

impl DnsChallengeHandler for MyDnsProvider {
    async fn handle_dns_challenge_create(&self, domain: &str, txt_value: &str) -> Result<String, String>;
    async fn handle_dns_challenge_cleanup(&self, domain: &str) -> Result<(), String>;
}
```

## API Reference

### P2PSession

**Connection Management:**
- `add_peer(peer_addr)` - Connect to a peer with persistent actor
- `list_peers()` - Get connected peer IDs
- `node_addr()` - Get this node's address

**DNS Challenge API:**
- `request_dns_challenge_create(peer_id, domain, txt_value)` - Request DNS challenge
- `request_dns_challenge_cleanup(peer_id, domain)` - Request cleanup

**Protocol Handles:**
- `take_inference_handle()` - Get handle for incoming inference requests
- `take_sni_proxy_handle()` - Get handle for SNI proxy streams

## Usage

### Daemon Example

```rust
// Create daemon session
let mut session = P2PSession::builder()
    .with_inference()
    .with_private_key(&daemon_key)
    .build().await?;

// Handle inference requests
let mut inference_handle = session.take_inference_handle().unwrap();
tokio::spawn(async move {
    while let Some(request) = inference_handle.next().await {
        // Process inference request
    }
});

// Connect to relay
let relay_addr = "...".parse()?;
session.add_peer(relay_addr).await?;
```

### Relay Example

```rust
// Create relay session with DNS handler
let session = P2PSession::builder()
    .with_sni_proxy()
    .with_dns_challenge()
    .with_dns_challenge_handler(cloudflare_handler)
    .build().await?;

// Handle SNI proxy streams
let sni_handle = session.take_sni_proxy_handle().unwrap();
// SNI streams are managed externally by the relay application
```

## Dependencies

- `iroh` - Core P2P networking and QUIC transport
- `tokio` - Async runtime and stream management
- `serde` / `serde_json` - Control message serialization
- `tracing` - Structured logging
- `dashmap` - Concurrent stream storage
- `uuid` - Request ID generation

## Security

- **Transport Encryption**: All communication via Iroh's encrypted QUIC
- **Node Identity**: Ed25519 keypairs for peer identification
- **Capability-Based**: Peers negotiate supported protocols
- **Stream Isolation**: Different protocols on separate streams
