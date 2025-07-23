# gate-daemon

The main Gate server binary that runs locally or on servers. Combines HTTP API server, P2P networking, and relay integration into a unified daemon.

## Responsibilities

- **HTTP Server**: Hosts OpenAI/Anthropic-compatible APIs on port 31145
- **P2P Integration**: Connects to other Gate nodes via Iroh
- **Relay Client**: Registers with relay for public HTTPS access
- **TLS Management**: Automated Let's Encrypt certificates with hot reload
- **Configuration**: JSON config file and environment variable support
- **State Management**: Coordinates between HTTP, P2P and local Inference layers

## Binary

```bash
gate
```

## Data Directory

Default locations:
- Linux: `~/.local/share/gate/`
- macOS: `~/Library/Application Support/com.hellas.gate/`
- Windows: `%APPDATA%\hellas\gate\`
