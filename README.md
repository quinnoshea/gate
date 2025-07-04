# Gate

Open-source AI gateway by Hellas AI. Provides unified access to AI models with support for local deployment, P2P sharing, and hosted SaaS.

## What is Gate?

Gate merges two previous Hellas AI projects:
- **hellas-cex**: Commercial API gateway with intelligent routing
- **private-gate**: P2P inference network with e2e encryption

The result is a flexible AI gateway that can be:
- Run locally as a daemon
- Deployed as a hosted service
- Connected P2P for sharing inference

## Quick Start

```bash
# Install and run locally
cargo install gate-daemon
gate serve

# API is now available at http://localhost:31145
curl http://localhost:31145/v1/chat/completions \
  -H "Authorization: Bearer $GATE_API_KEY" \
  -d '{"model": "gpt-4", "messages": [{"role": "user", "content": "Hello!"}]}'
```

## Architecture

Gate uses a modular Rust workspace:

### Core Infrastructure
- `gate-core`: Trait abstractions (StateBackend, Plugin system)
- `gate-http`: HTTP server and client (Axum-based)
- `gate-sqlx`: Database backends (PostgreSQL, SQLite)

### P2P & Networking
- `gate-p2p`: Iroh-based P2P connectivity
- `gate-relay`: HTTPS relay for public access

### User Interfaces
- `gate-frontend`: Web dashboard (Yew/WASM)
- `gate-gui`: Desktop app (Tauri)
- `gate-chat-ui`: Reusable chat component

### Main Binary
- `gate-daemon`: Server combining all components

## Features

### 🔌 Extensible Plugin System
```rust
impl GatePlugin for MyPlugin {
    async fn init(&mut self, ctx: PluginContext) -> Result<()> {
        // Add custom logic
    }
}
```

### 🌐 Multi-Provider Support
- OpenAI, Anthropic, DeepSeek, Together.ai
- Custom providers via plugin
- Automatic failover and load balancing

### 🔒 P2P Inference Sharing
- Share GPU resources with friends
- End-to-end encryption
- No central servers required

### 🚀 Production Ready
- SQLite for single-node deployments
- PostgreSQL for scale
- Prometheus metrics
- OpenTelemetry tracing

## Development

### Prerequisites
- Rust 1.75+
- Nix (optional, for development shell)

### Building
```bash
make build       # Build all crates
make test        # Run tests
make pre-commit  # Run before committing
```

### Running Development Server
```bash
make dev         # Debug build
make run         # Start server
make frontend-dev # Start frontend dev server
```

## Configuration

Create `gate.toml`:
```toml
[server]
bind = "127.0.0.1:31145"

[database]
url = "sqlite://gate.db"

[providers.openai]
api_key = "$OPENAI_API_KEY"
```

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Commercial Support

For hosted Gate, enterprise features, and support: [hellas.ai](https://hellas.ai)