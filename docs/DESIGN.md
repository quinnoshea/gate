# Gate - System Design

## Architecture Overview

Gate is a distributed system consisting of several interacting components that provide secure, peer-to-peer access to AI compute resources. The system is designed around a local daemon that acts as both a client and server in a peer-to-peer network.

## Component Architecture

### Core Daemon Library (`crates/daemon/`)
The main library that coordinates all system functionality. Can be embedded in other applications (GUI, CLI) or run as standalone binary.

**Responsibilities:**
- HTTP API server (OpenAI-compatible endpoints)
- P2P networking and message handling  
- Request routing and response forwarding
- Configuration management and hot-reloading
- Provider integration (Ollama, LM Studio)
- Web frontend serving
- Unified HTTP processing for local and remote requests

**Key modules:**
- `lib.rs` - Public library interface for embedding
- `daemon.rs` - Core daemon struct and lifecycle management
- `http_server.rs` - Axum-based HTTP API server ([PLAN.md#http-api-server](PLAN.md#http-api-server))
- `p2p_manager.rs` - Iroh networking and peer communication ([PLAN.md#p2p-networking](PLAN.md#p2p-networking))
- `request_router.rs` - Route requests between local/remote providers ([PLAN.md#request-routing](PLAN.md#request-routing))
- `config_manager.rs` - Configuration loading and validation ([PLAN.md#configuration-management](PLAN.md#configuration-management))


### P2P Networking (`crates/p2p/`)
Handles all peer-to-peer communication using Iroh.

**Protocol layers:**
- **Transport**: Iroh QUIC streams for encrypted communication
- **Protocol Negotiation**: Explicit `protocol::v1` header on stream establishment
- **Stream Multiplexing**: Multiple concurrent streams over single connection (control, HTTP proxy, etc.)
- **Node Discovery**: DHT-based peer discovery and manual peer addition
- **Message Protocol**: JSON-based control messages over dedicated streams
- **Connection Management**: Connection pooling, heartbeat, reconnection logic

**Key types:**
```rust
// Defined in crates/core/src/types.rs
pub struct NodeId(pub PublicKey); // 32-byte Ed25519 public key
pub struct PeerInfo {
    pub node_id: NodeId,
    pub last_seen: SystemTime,
    pub connection_status: ConnectionStatus,
    pub capabilities: Option<Capabilities>,
    pub addresses: Vec<SocketAddr>,
}

pub enum ConnectionStatus {
    Connected,
    Connecting, 
    Disconnected,
    Failed(String),
}
```

### RPC Interface (`crates/rpc/`)
JSON-RPC interface for daemon control and monitoring.

**Transport**: HTTP POST to `/rpc` endpoint, Server-Sent Events for real-time updates

**Method categories:**
- **Node Management**: `node_status()`, `node_shutdown()`, `node_restart()`
- **Peer Management**: `add_peer()`, `remove_peer()`, `list_peers()`, `peer_info()`
- **Configuration**: `get_config()`, `update_config()`, `validate_config()`
- **Providers**: `list_providers()`, `add_provider()`, `test_provider()`
- **Monitoring**: `get_metrics()`, `get_logs()`, `request_history()`
- **Identity**: `generate_keypair()`, `export_identity()`, `import_identity()`

**Complete method signatures**: See [crates/rpc/src/methods.rs](crates/rpc/src/methods.rs) (*Implementation planned in [PLAN.md#control-interface](PLAN.md#control-interface)*)

### Core Types (`crates/core/`)
Shared data structures and protocol definitions.

**Key types:**
```rust
// Configuration schema
#[derive(Serialize, Deserialize)]
pub struct Configuration {
    pub node: NodeConfig,
    pub network: NetworkConfig, 
    pub providers: Vec<ProviderConfig>,
    pub security: SecurityConfig,
    pub web: WebConfig,
}

pub struct NodeConfig {
    pub identity: Identity,           // Private key, public key
    pub data_dir: PathBuf,           // ~/.config/gate/ 
    pub log_level: String,           // debug, info, warn, error
}

pub struct NetworkConfig {
    pub p2p_port: u16,               // Default: 41145
    pub http_port: u16,              // Default: 31145  
    pub control_port: u16,           // Default: 8145
    pub trusted_peers: Vec<NodeId>,  // Allowed to make requests
    pub max_connections: usize,      // Connection pool size
}

pub struct ProviderConfig {
    pub name: String,                // "ollama", "lmstudio"
    pub url: Url,                    // http://localhost:11434
    pub enabled: bool,
    pub models: Vec<String>,         // Available models
    pub health_check_interval: Duration,
}

// Stream purpose identification
#[derive(Serialize, Deserialize)]
pub enum StreamType {
    NodeControl,      // Node-to-node control messages
    RelayControl,     // Node-to-relay control messages
    TlsProxy,         // Raw TLS bytes from browser HTTPS
}

// Node-to-node control messages
#[derive(Serialize, Deserialize)]
pub struct NodeMessage {
    pub version: u8,                 // Protocol version (1)
    pub id: MessageId,               // Request correlation ID
    pub timestamp: u64,              // Unix timestamp
    pub payload: NodeMessagePayload, // Message content
}

pub enum NodeMessagePayload {
    Handshake { capabilities: Capabilities },
    HandshakeResponse { accepted: bool, capabilities: Option<Capabilities> },
    CapabilityRequest,
    CapabilityResponse { capabilities: Capabilities },
    Ping,
    Pong,
}

// Node-to-relay control messages
#[derive(Serialize, Deserialize)]
pub struct RelayMessage {
    pub version: u8,                 // Protocol version (1)
    pub id: MessageId,               // Request correlation ID
    pub timestamp: u64,              // Unix timestamp
    pub payload: RelayMessagePayload, // Message content
}

pub enum RelayMessagePayload {
    DomainRequest { node_id: NodeId },
    DomainResponse { domain: String, status: String },
    DnsChallenge { domain: String, token: String },
    DnsChallengeResponse { record: String, status: String },
    ActivateDomain { domain: String },
    ActivationResponse { status: String, relay_endpoints: Vec<String> },
}

pub struct Capabilities {
    pub node_id: NodeId,
    pub supported_models: Vec<ModelInfo>,
    pub max_context_length: Option<u32>,
    pub supports_streaming: bool,
    pub load_factor: f32,            // 0.0 = idle, 1.0 = fully loaded
}
```

### Web Frontend (`crates/web/`)
Yew-based single-page application for daemon management.

**Architecture:**
- **Framework**: Yew (Rust -> WebAssembly)
- **Communication**: HTTP + Server-Sent Events to RPC interface for real-time updates
- **Routing**: Browser-based routing with Yew Router
- **State Management**: Yew contexts for shared state
- **Styling**: CSS modules or Tailwind CSS

**Pages and components:**
- **Dashboard** (`/`): Node status, recent requests, system metrics
- **Peers** (`/peers`): Connected peers, add/remove peers, connection status
- **Configuration** (`/config`): Node settings, provider management
- **Providers** (`/providers`): Local provider status, model availability
- **Logs** (`/logs`): Real-time log streaming and filtering
- **Setup** (`/setup`): First-time setup wizard

**Key components:**
```rust
// crates/web/src/components/
pub struct Dashboard {
    pub node_status: NodeStatus,
    pub recent_requests: Vec<RequestInfo>,
    pub peer_count: usize,
}

pub struct PeerManager {
    pub peers: Vec<PeerInfo>,
    pub connection_states: HashMap<NodeId, ConnectionStatus>,
}
``` 

### CLI Tool (`crates/cli/`)
Command-line interface for daemon management.

**Command structure:**
```bash
gate daemon [--config PATH] [--background]  # Start daemon
gate stop                                   # Stop daemon  
gate status                                 # Show status
gate peers add <NODE_ID> [ADDRESS]         # Add peer
gate peers list                             # List peers
gate config show                            # Show config
gate config validate                       # Validate config
gate setup                                 # Interactive setup
gate logs [--follow] [--level LEVEL]       # View logs
```

### Relay Server (`crates/relay/`)
Public HTTPS proxy for browser-compatible endpoints.

**Functionality:**
- **DNS Management**: Provision subdomains via Cloudflare API
- **Node-Relay Communication**: Control streams over Iroh for DNS challenge coordination
- **TLS Proxying**: Route raw TLS bytes to nodes via separate Iroh streams
- **Load Balancing**: Multiple relay instances with anycast IPs

**Key components:**
- `dns_manager.rs` - Cloudflare API integration
- `relay_protocol.rs` - Node-relay control message handling
- `tls_proxy.rs` - SNI-based TLS proxying
- `node_registry.rs` - Track active nodes and their domains

**Iroh Stream Multiplexing:**
- Single Iroh connection per node-relay pair
- Control stream for domain/DNS management
- Multiple TLS proxy streams for browser connections
- Stream type negotiation on establishment

## Data Flow

### Local Inference Request
1. Client → `POST localhost:31145/v1/chat/completions`
2. HTTP server validates request, checks rate limits
3. Request router determines target (local provider)
4. HTTP client forwards to provider (e.g., `localhost:11434/v1/chat/completions`)
5. Provider response streamed back to client

### Remote Inference Request  
1. Client → `POST localhost:31145/v1/chat/completions` with `X-Target-Node: <node_id>`
2. HTTP server validates request, checks if peer is trusted
3. P2P manager opens control stream to target node with `protocol::v1`
4. HTTP request forwarded as structured message over Iroh stream
5. Remote node receives message, injects into same HTTP pipeline as local requests
6. Response streamed back through P2P connection

### Public HTTPS Request
1. Browser → `https://{node-id}.private.hellas.ai/v1/chat/completions`
2. Relay server extracts SNI from TLS ClientHello, looks up node
3. TLS proxy stream opened to node via Iroh (separate from control stream)
4. Raw TLS bytes proxied transparently to node
5. Node terminates TLS with its private key, extracts HTTP request
6. HTTP request processed through same pipeline as local requests

## Security Model

### Authentication
- **Node Identity**: Ed25519 keypairs, public key as node address
- **Iroh Transport Security**: All P2P communication encrypted by Iroh QUIC
- **Trust Lists**: Explicit configuration of trusted peers
- **Relay Authentication**: Control messages authenticated via node signatures

### Encryption
- **Transport**: Iroh provides QUIC encryption for all P2P communication
- **HTTPS**: TLS termination at node for public endpoints using `rustls` library
- **At Rest**: Private keys encrypted with system keyring

### Authorization  
- **Peer Access**: Only trusted peers can make inference requests
- **Local API**: Binds to localhost only, requires local access
- **Control Interface**: Optional authentication token

### Rate Limiting and Overload Protection
- **Per-peer rate limiting**: Prevent individual peers from overwhelming nodes
- **Global request throttling**: Limit total concurrent inference requests
- **Provider protection**: Rate limit requests to local inference engines
- **Graceful degradation**: Return HTTP 429 (Too Many Requests) when limits exceeded
- **Resource monitoring**: Track CPU, memory, and network usage for adaptive limiting

## Error Handling Strategy

### Library Crates
- **Error types**: Each library crate defines its own error type using `thiserror`
- **Error propagation**: Use `Result<T, CrateError>` for all fallible operations
- **Error conversion**: Implement `From` traits for seamless error conversion between crates

### Binary Applications
- **Application errors**: CLI and daemon binaries may use `anyhow` for application-level error handling
- **User-facing errors**: Convert technical errors to user-friendly messages
- **Error logging**: Use `tracing` for structured error logging

### Examples and Tests
- **Simplified handling**: Examples and test code may use `anyhow` for brevity
- **Error demonstration**: Examples should show proper error handling patterns

## Configuration Management

### File Locations
- **Config Dir**: Platform-standard locations via `directories` crate
  - Linux: `~/.config/gate/`
  - macOS: `~/Library/Application Support/gate/`
  - Windows: `%APPDATA%\gate\`

### File Structure
```
~/.config/gate/
├── config.json          # Main configuration
├── identity.key         # Node private key (encrypted)
├── peers.json           # Peer information cache (mutable JSON state)  
├── logs/                # Request logs (if enabled)
│   ├── requests.jsonl   # Inference request history
│   └── daemon.log       # Daemon logs
└── web/                 # Web frontend assets
    └── index.html       # Yew-compiled frontend
```

### Hot Reloading
Configuration changes automatically reload without daemon restart (where safe). Identity and network port changes require restart.

## Platform Support

### Current Targets
- **Desktop**: macOS (Intel/Apple Silicon), Linux (x86_64), Windows (x86_64)
- **Web**: All browsers supporting WebAssembly
- **Architecture**: Native binaries + WASM components

### Future Targets  
- **Mobile**: iOS app, Android app (using same Rust core)
- **Server**: Docker containers, cloud deployment
- **Embedded**: Raspberry Pi, embedded Linux devices

## Testing Strategy

### Unit Tests
- **Core types**: Serialization, validation, conversion
- **Crypto**: Key generation, signing, verification  
- **Protocol**: Message parsing, handshake flows
- **Configuration**: Loading, validation, migration

### Integration Tests
- **P2P**: Full node-to-node communication
- **HTTP API**: OpenAI compatibility, streaming
- **Provider Integration**: Ollama, LM Studio compatibility
- **RPC**: Control interface functionality

### End-to-End Tests
- **Multi-node**: Complex request routing scenarios
- **Web Frontend**: Browser automation with Playwright
- **CLI**: Command-line interface functionality
- **Relay**: Public HTTPS proxy behavior

### Performance Tests
- **Throughput**: Concurrent requests, streaming performance
- **Latency**: Request routing overhead measurement
- **Memory**: Long-running daemon stability
- **Network**: P2P connection scaling

## Development Guidelines

### Cargo Workspace Management
- **Shared Dependencies**: All common dependencies defined in `[workspace.dependencies]`
- **Inheritance**: Sub-crates use `dependency.workspace = true` to inherit versions
- **Minimal Dependencies**: Only add dependencies when actually needed, not speculatively
- **Workspace Configuration**: Sub-crates inherit package metadata, lints, and configuration from workspace

### Implementation Philosophy
- **Minimal, Focused Implementation**: Only implement exactly what is requested
- **No Speculative Features**: Don't add "helpful" additions or conveniences not asked for
- **Dependency Hygiene**: Add dependencies incrementally as features require them

## Development Phases

Development follows a "fast as we can correctly" approach with AI assistance, prioritizing correctness over speed.

**Phase 1**: Core infrastructure and basic P2P communication
**Phase 2**: HTTP API and local provider integration  
**Phase 3**: Web frontend and control interface
**Phase 4**: Public relay system and HTTPS endpoints
**Phase 5**: Mobile applications and advanced features

*Detailed implementation plan and task breakdown: [PLAN.md](PLAN.md)*