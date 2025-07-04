# gate-tlsforward

TLS forwarding service that enables public web access to P2P Gate nodes. Provides both server (standalone TLS forward) and client (node integration) functionality.

## Responsibilities

- **TLS Forwarding**: Proxy HTTPS traffic to P2P nodes without terminating TLS
- **DNS Management**: Automated DNS records via Cloudflare API
- **Certificate Support**: Let's Encrypt integration for public domains
- **SNI Routing**: Routes requests based on TLS Server Name Indication
- **Rate Limiting**: Protects relay from abuse

## Binary

```bash
gate-tlsforward serve # Run standalone TLS forward server
```

## Organization

```
src/
├── client/          # Client library for nodes
│   ├── cert.rs     # Certificate management
│   ├── tlsforward.rs    # TLS forward registration
│   └── handler.rs  # TLS forwarding handler
├── server/          # TLS forward server implementation
│   ├── proxy.rs    # HTTPS proxy logic
│   ├── dns.rs      # Cloudflare DNS management
│   └── state.rs    # Connection tracking
└── common/          # Shared protocol definitions
```

## Features

- `default`: Both client and server
- `client`: Node-side relay integration only
- `server`: Standalone TLS forward server only

## Architecture

### How It Works

1. **Node Registration**:
   - Node connects to TLS forward server via P2P
   - Requests DNS challenge for Let's Encrypt
   - Obtains certificate for `{node-id}.private.hellas.ai`
   - Registers domain with TLS forward server

2. **HTTPS Routing**:
   - Browser connects to `https://{node-id}.private.hellas.ai`
   - TLS forward server extracts node ID from SNI
   - Opens TLS forwarding stream to node via P2P
   - Node terminates TLS with its certificate
   - TLS forward server proxies encrypted bytes bidirectionally

3. **Security Model**:
   - Private keys never leave nodes
   - TLS forward server only sees encrypted traffic
   - Nodes control their own certificates

## Client Usage

```rust
use gate_tlsforward::{TlsForwardClient, CertificateManager};

let cert_manager = CertificateManager::new(state_dir);
let tls_forward_client = TlsForwardClient::new(endpoint, cert_manager);

// Register with TLS forward server
tls_forward_client.register().await?;
```

## Server Configuration

```toml
[tlsforward]
bind = "0.0.0.0:443"
domain = "private.hellas.ai"

[cloudflare]
api_token = "xxx"
zone_id = "yyy"
```

## Dependencies

Client:
- `instant-acme`: Let's Encrypt client
- `rcgen`: Self-signed certificate generation
- `gate-p2p`: P2P connectivity

Server adds:
- `cloudflare`: DNS API client
- `axum`: HTTP server
- `x509-parser`: Certificate validation

## Protocols

- **TLS Forward**: `/gate.tlsforward.v1.TlsForward/1.0`
- **HTTP API**: `/gate.tlsforward.v1.Http/1.0`

## Risks

- **DNS Propagation**: Certificate issuance depends on DNS updates
- **Rate Limits**: Let's Encrypt and Cloudflare API limits
- **Single Point**: TLS forward server is centralized (mitigated by self-hosting)