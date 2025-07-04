# gate-daemon

The main Gate server binary that runs locally or on servers. Combines HTTP API server, P2P networking, and relay integration into a unified daemon.

## Responsibilities

- **HTTP Server**: Hosts OpenAI/Anthropic-compatible APIs on port 31145
- **P2P Integration**: Connects to other Gate nodes via Iroh
- **Relay Client**: Registers with relay for public HTTPS access
- **TLS Management**: Automated Let's Encrypt certificates with hot reload
- **Configuration**: YAML/TOML config file and environment variable support
- **State Management**: Coordinates between HTTP, P2P, and storage layers

## Binary

```bash
gate serve # Start the daemon
```

## Organization

```
src/
├── main.rs         # Entry point, service initialization
├── config.rs       # Configuration structures
├── context.rs      # Request context implementation
├── state.rs        # Server state coordination
├── state_dir.rs    # Data directory management
├── tls_reload.rs   # Hot-reloadable TLS acceptor
└── services/
    └── relay.rs    # Relay registration service
```

## Configuration

### File Format (gate.toml)
```toml
[server]
bind = "127.0.0.1:31145"

[database]
url = "sqlite://gate.db"

[p2p]
enabled = true
listen_port = 4919

[relay]
enabled = true
endpoint = "https://relay.hellas.ai"
```

### Environment Variables
- `GATE_CONFIG`: Path to config file
- `DATABASE_URL`: Override database connection
- `RUST_LOG`: Logging configuration

## Key Components

### ServerState
Coordinates all daemon components:
- HTTP app state
- P2P node handle
- Certificate manager
- Background services

### TLS Hot Reload
Automatically reloads certificates without restart:
```rust
let acceptor = ReloadableTlsAcceptor::new(cert_path, key_path);
// Watches files and reloads on change
```

### Relay Service
Background task that:
- Registers node with relay
- Maintains DNS records
- Handles certificate renewals

## Dependencies

- `axum`: HTTP server framework
- `iroh`: P2P networking
- `gate-core/http/p2p/relay`: Gate subsystems
- `config`: Configuration management
- `rustls`: TLS implementation
- `tokio`: Async runtime

## Data Directory

Default locations:
- Linux: `~/.local/share/gate/`
- macOS: `~/Library/Application Support/com.hellas.gate/`
- Windows: `%APPDATA%\hellas\gate\`

Contains:
- `gate.db`: SQLite database
- `certs/`: TLS certificates
- `iroh/`: P2P node data

## Risks

- **Port Conflicts**: Default ports (31145, 4919) may be in use
- **Certificate Renewal**: Relay service must stay connected
- **State Coordination**: Complex interactions between subsystems