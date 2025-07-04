# gate-p2p

P2P networking layer for Gate using Iroh. Provides encrypted peer-to-peer communication with NAT traversal for sharing AI inference between nodes.

## Responsibilities

- **P2P Router**: Protocol multiplexing over Iroh connections
- **Stream Utilities**: Bidirectional stream helpers for protocols
- **Node Identity**: Ed25519-based node identification
- **NAT Traversal**: Automatic hole-punching via Iroh's QUIC transport

## Organization

```
src/
├── router.rs  # Protocol router and configuration
└── stream.rs  # Stream utilities for bidirectional communication
```

## Features

- Native only (not available in WASM)
- Uses Iroh's QUIC-based transport
- Automatic encryption between peers

## Usage

```rust
use gate_p2p::{create_router, RouterConfig};
use iroh::{Endpoint, SecretKey};

// Create P2P endpoint
let secret_key = SecretKey::generate();
let endpoint = Endpoint::builder()
    .secret_key(secret_key)
    .bind()
    .await?;

// Create protocol router
let config = RouterConfig::default();
let router = create_router(endpoint, config).await?;
```

## Key Components

### RouterConfig
Configuration for P2P protocols:
- Protocol handlers registration
- Connection settings
- Timeout configurations

### Stream Utilities
Helpers for working with Iroh's bidirectional streams:
- Framed message passing
- Error handling
- Graceful shutdown

## Dependencies

- `iroh`: Core P2P networking (native only)
- `tokio`: Async runtime with IO utilities
- `futures`: Stream processing

## Protocol Design

Gate uses multiple protocols over P2P:
- **Control**: Basic connectivity and capability exchange
- **Inference**: AI model request/response
- **Relay**: DNS and certificate management
- **TLS Forwarding**: HTTPS proxy over P2P

Each protocol is identified by ALPN string.

## Risks

- **Native Only**: No WASM support limits browser usage
- **Network Dependencies**: Requires UDP for QUIC
- **Key Management**: Node identity must be persisted