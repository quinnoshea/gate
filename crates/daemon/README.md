# hellas-gate-daemon

Gate daemon that provides private AI inference endpoints through P2P networking.

## Overview

The daemon provides secure, private access to AI models by:
- Connecting to relay nodes via P2P network
- Terminating TLS and forwarding HTTP requests to local server
- Managing LetsEncrypt certificates via DNS challenges
- Serving OpenAI-compatible API on local HTTP endpoint

## Current Status

**✅ Implemented:**
- P2P session with inference protocol support
- TLS termination with self-signed certificates
- HTTP server with OpenAI-compatible API
- TLS bridge for HTTPS request processing
- LetsEncrypt ACME integration with DNS challenge support
- P2P DNS challenge resolver (communicates with relay)
- Upstream provider integration

**🔄 In Progress:**
- Full ACME certificate workflow implementation
- SNI stream management (externalized from P2P crate)

## Architecture

### Core Components

- **GateDaemon**: Main orchestrator managing all services
- **HttpServer**: Axum-based server with OpenAI-compatible endpoints
- **TlsHandler**: TLS termination using rustls with certificate management
- **TlsBridge**: Connects TLS termination to HTTP forwarding
- **LetsEncryptManager**: ACME client with challenge resolver abstraction
- **P2PDnsChallengeResolver**: DNS challenge resolver using P2P communication

### TLS & Certificate Management

The daemon supports two certificate modes:

**Self-Signed Certificates (Default):**
```rust
let tls_handler = TlsHandler::new(&node_id, &private_key)?;
```

**LetsEncrypt with DNS Challenges:**
```rust
let le_manager = LetsEncryptManager::new(le_config).await?;
let resolver = P2PDnsChallengeResolver::new(p2p_session, relay_peer_id);
le_manager.request_certificate(Arc::new(resolver)).await?;
```

### HTTP API

OpenAI-compatible endpoints:
- `GET /health` - Health check
- `GET /status` - Node status and info
- `POST /v1/chat/completions` - Chat completions (forwarded to upstream)
- `GET /peers` - List connected peers
- `POST /peers/:id/connect` - Connect to peer

### P2P Integration

The daemon connects to relay nodes for public access:

```rust
// Create P2P session
let session = P2PSession::builder()
    .with_inference()
    .with_private_key(&identity)
    .build().await?;

// Connect to relay
let relay_addr = "...".parse()?;
session.add_peer(relay_addr).await?;

// Handle HTTPS requests from relay
if let Some(tls_bridge) = &daemon.tls_bridge {
    let response = tls_bridge.process_https_bytes(https_data).await?;
}
```

## Configuration

The daemon uses a layered configuration system:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub http: HttpConfig,      // Local HTTP server settings
    pub p2p: P2PConfig,        // P2P networking settings
    pub tls: TlsConfig,        // TLS and certificate settings
    pub upstream: UpstreamConfig, // AI provider settings
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub bind_addr: SocketAddr,
    pub enabled: bool,
    pub letsencrypt: Option<LetsEncryptConfig>, // ACME settings
}
```

## LetsEncrypt Integration

The daemon can automatically obtain certificates via ACME DNS-01 challenges:

```rust
let le_config = LetsEncryptConfig {
    cert_dir: PathBuf::from("./certs"),
    email: "admin@example.com".to_string(),
    staging: true, // Use Let's Encrypt staging for testing
    domains: vec!["mynode.private.hellas.ai".to_string()],
    auto_renew: true,
};

// P2P DNS challenge resolver communicates with relay
let resolver = P2PDnsChallengeResolver::new(p2p_session, relay_peer_id);
le_manager.request_certificate(Arc::new(resolver)).await?;
```

The resolver sends DNS challenge requests to the relay via P2P control protocol.

## DNS Challenge Flow

1. Daemon requests certificate from LetsEncrypt
2. LetsEncrypt provides DNS challenge (domain + TXT value)
3. `P2PDnsChallengeResolver` sends challenge to relay via P2P
4. Relay creates DNS TXT record using Cloudflare API
5. LetsEncrypt validates challenge and issues certificate
6. Cleanup request sent to relay to remove TXT record

## Usage

### Basic Usage

```rust
use hellas_gate_daemon::{GateDaemon, DaemonConfig};

let config = DaemonConfig::default();
let identity = load_or_generate_identity()?;

let mut daemon = GateDaemon::new(config, identity)?;
let node_addr = daemon.run().await?;

println!("Daemon running at: {}", node_addr);
```

### With LetsEncrypt

```rust
let mut config = DaemonConfig::default();
config.tls.letsencrypt = Some(LetsEncryptConfig {
    domains: vec!["mynode.private.hellas.ai".to_string()],
    email: "admin@example.com".to_string(),
    staging: true,
    cert_dir: PathBuf::from("./certs"),
    auto_renew: true,
});

let daemon = GateDaemon::new(config, identity)?;
```

## Dependencies

- `axum` - HTTP server framework
- `tokio-rustls` - TLS implementation
- `acme-lib` - ACME protocol client
- `reqwest` - HTTP client for upstream providers
- `hellas-gate-p2p` - P2P networking
- `rcgen` - Self-signed certificate generation

## Security

- **TLS Termination**: All public traffic encrypted via TLS
- **Private Keys**: Node identity never transmitted
- **Local-Only API**: HTTP server binds to localhost by default
- **Certificate Management**: Automatic renewal with secure key storage
