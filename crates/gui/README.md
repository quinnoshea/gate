# Gate GUI with Embedded Daemon

This is the Tauri desktop application for Gate that includes an embedded daemon server.

## Architecture

The GUI app runs the Gate daemon as an embedded library within the same process, rather than spawning it as a separate binary. This provides:

- Single process deployment
- Direct access to daemon internals
- Shared memory and resources
- Better error handling

## Tauri Commands

The frontend (written in Rust/Yew) can access the daemon state directly through Tauri commands:

### Using from Rust Frontend

```rust
use gate_frontend_tauri::tauri_api::*;
use wasm_bindgen_futures::spawn_local;

// In your Yew component
spawn_local(async {
    // Start daemon with default config
    match start_daemon(None).await {
        Ok(msg) => tracing::info!("{}", msg),
        Err(e) => tracing::error!("Failed to start daemon: {}", e),
    }
    
    // Start with custom config
    let config = DaemonConfig {
        port: 3001,
        host: "127.0.0.1".to_string(),
        database_url: Some("sqlite:gate.db".to_string()),
    };
    start_daemon(Some(config)).await;
    
    // Get daemon status
    match get_daemon_status().await {
        Ok(status) => {
            tracing::info!("Daemon running: {}", status.running);
            if let Some(addr) = status.listen_address {
                tracing::info!("Listening on: {}", addr);
            }
            tracing::info!("Has upstreams: {}", status.has_upstreams);
        }
        Err(e) => tracing::error!("Failed to get status: {}", e),
    }
    
    // Get runtime config
    match get_daemon_runtime_config().await {
        Ok(config) => {
            tracing::info!("Listen address: {}", config.listen_address);
            tracing::info!("Database: {}", config.database_url);
            tracing::info!("Upstreams: {}", config.upstream_count);
            tracing::info!("Auth enabled: {}", config.auth_enabled);
            tracing::info!("WebAuthn enabled: {}", config.webauthn_enabled);
        }
        Err(e) => tracing::error!("Failed to get config: {}", e),
    }
    
    // Check if running
    match daemon_status().await {
        Ok(is_running) => tracing::info!("Is running: {}", is_running),
        Err(e) => tracing::error!("Failed to check status: {}", e),
    }
    
    // Stop daemon
    stop_daemon().await;
    
    // Restart with new config
    restart_daemon(Some(config)).await;
});
```

### Available Commands

- `start_daemon(config: Option<DaemonConfig>)` - Start the daemon
- `stop_daemon()` - Stop the daemon  
- `daemon_status()` - Check if daemon is running (returns bool)
- `get_daemon_config()` - Get current daemon config
- `get_daemon_status()` - Get runtime status (running, address, upstreams)
- `get_daemon_runtime_config()` - Get full runtime configuration
- `restart_daemon(config: Option<DaemonConfig>)` - Restart with optional new config

All commands return `Result<T, String>` for error handling.

## Auto-start

The daemon automatically starts when the GUI app launches. It will:
- Use default configuration (port 3000, localhost only)
- Create an in-memory SQLite database
- Disable features not needed for GUI (WebAuthn, Let's Encrypt, TLS forwarding)

## Shutdown

The daemon automatically stops when the GUI window is closed, ensuring clean shutdown.

## Development

To run the GUI app in development mode:

```bash
cd crates/gui
cargo tauri dev
```

To build for production:

```bash
cargo tauri build
```