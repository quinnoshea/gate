# hellas-gate-relay

TLS SNI proxy and DNS management for Gate P2P network - enables public HTTPS access to private nodes.

## Overview

The relay provides public infrastructure for the Gate network by:
- Proxying raw TLS bytes to target nodes via P2P connections
- Managing DNS records for node discovery (`{node_id}.private.hellas.ai`)
- Handling DNS challenges for LetsEncrypt certificate automation
- Operating public HTTPS endpoints on behalf of private nodes

## Current Status

**✅ Implemented:**
- SNI extraction from TLS ClientHello packets
- P2P session with SNI proxy and DNS challenge support
- Cloudflare DNS API integration for record management
- DNS challenge handler for ACME certificate requests
- Node registry for domain-to-peer mapping
- TLS proxy for raw byte forwarding

**🔄 In Progress:**
- Complete SNI proxy workflow integration
- Automatic node discovery and DNS provisioning

## Architecture

### Core Components

- **RelayServer**: Main orchestrator managing HTTPS listener and P2P session
- **TlsProxy**: Forwards raw TLS bytes between browsers and Gate nodes
- **SniExtractor**: Parses SNI from TLS ClientHello to identify target nodes
- **DnsManager**: Cloudflare API integration for DNS record management
- **CloudflareDnsChallengeHandler**: Processes ACME DNS challenges via P2P
- **NodeRegistry**: Maps domains to peer connections and node information

### DNS Challenge Integration

The relay responds to DNS challenge requests from daemons:

```rust
// P2P session with DNS challenge handler
let cloudflare_handler = Arc::new(CloudflareDnsChallengeHandler::new(dns_manager));

let session = P2PSession::builder()
    .with_sni_proxy()
    .with_dns_challenge()
    .with_dns_challenge_handler(cloudflare_handler)
    .build().await?;
```

When a daemon requests a DNS challenge:
1. Daemon sends `DnsChallengeCreate` via P2P control protocol
2. Relay creates TXT record using Cloudflare API
3. Relay responds with `DnsChallengeResponse` indicating success/failure
4. After ACME validation, daemon sends `DnsChallengeCleanup`

### SNI Proxy Flow

1. Browser connects to `{node_id}.private.hellas.ai:443`
2. `SniExtractor` parses TLS ClientHello to get SNI domain
3. Domain parsed to extract node ID: `abcd1234...private.hellas.ai` → `abcd1234...`
4. `NodeRegistry` looks up P2P connection for node ID
5. `TlsProxy` forwards raw TLS bytes to target node via P2P
6. Node terminates TLS, processes HTTP, sends response back through P2P
7. Relay forwards response to browser

### Cloudflare DNS Management

All Cloudflare-specific code is contained in the `cloudflare_dns` module:

```rust
use hellas_gate_relay::cloudflare_dns::CloudflareDnsChallengeHandler;

impl DnsChallengeHandler for CloudflareDnsChallengeHandler {
    async fn handle_dns_challenge_create(&self, domain: &str, txt_value: &str) -> Result<String, String>;
    async fn handle_dns_challenge_cleanup(&self, domain: &str) -> Result<(), String>;
}
```

## Configuration

### Environment Variables

Required for Cloudflare integration:
```bash
export CLOUDFLARE_API_TOKEN=your_api_token
export CLOUDFLARE_ZONE_ID=your_zone_id
export RELAY_BASE_DOMAIN=private.hellas.ai  # Optional, defaults to private.hellas.ai
```

### Relay Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    pub https: HttpsConfig,     // Public HTTPS listener
    pub p2p: P2PConfig,         // P2P networking
    pub dns: DnsConfig,         // DNS management settings
}
```

## Usage

### Basic Relay

```rust
use hellas_gate_relay::{RelayServer, RelayConfig};

let config = RelayConfig::default();
let identity = load_relay_identity()?;

let relay = RelayServer::new(config, identity).await?;
relay.run().await?;
```

### Adding Peers

```rust
// Connect to a Gate daemon and register domain
let peer_addr = "daemon_peer_id@127.0.0.1:31145";
let domain = "abcd1234.private.hellas.ai";

let gate_id = relay.add_peer(peer_addr, domain).await?;
println!("Registered daemon {} with domain {}", gate_id, domain);
```

## DNS Management

The relay automatically manages DNS records for connected nodes:

- **A/AAAA Records**: Point to relay's public IP addresses
- **TXT Records**: Created/cleaned for ACME DNS challenges
- **TTL**: Short TTLs (2-5 minutes) for faster updates

### Supported Operations

- `provision_subdomain(node_id)` - Create DNS records for new node
- `create_dns_challenge(domain, txt_value)` - Create ACME challenge record
- `cleanup_dns_challenge(record_id)` - Remove challenge record
- `cleanup_subdomain(domain)` - Remove all records for domain

## Security

- **TLS Passthrough**: Relay never decrypts end-to-end TLS traffic
- **P2P Encryption**: All relay-to-node communication encrypted via Iroh
- **DNS Security**: Cloudflare API tokens with minimal required permissions
- **Node Isolation**: Each node accessible only via its unique subdomain

## Dependencies

- `hellas-gate-p2p` - P2P networking and control protocol
- `reqwest` - HTTP client for Cloudflare API
- `trust-dns-resolver` - DNS resolution for challenge validation
- `rustls` / `tokio-rustls` - TLS certificate management
- `bytes` - Efficient byte manipulation for TLS proxy

## Deployment

The relay requires:
- **Public IP**: For incoming HTTPS connections on port 443
- **DNS Control**: Cloudflare API access for the target domain
- **P2P Connectivity**: Ability to reach Gate daemon nodes
- **Certificate Management**: TLS certificate for the relay domain

Typical deployment: VPS with public IP, Cloudflare DNS management, systemd service.
