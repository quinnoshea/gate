# gate-core

Core abstractions and types for Gate. Defines the trait-based architecture that enables Gate's extensibility across native and WASM environments.

## Responsibilities

- **StateBackend trait**: Abstraction for persistent storage (users, API keys, usage tracking)
- **Plugin system**: `GatePlugin` trait and hook registry for extending functionality
- **Common types**: Shared data structures (User, ApiKey, Provider, Model, etc.)
- **Error handling**: Unified error type with proper `#[from]` conversions
- **WebAuthn**: Hardware authentication support

## Key Traits

### StateBackend
```rust
#[async_trait]
pub trait StateBackend: Send + Sync {
    async fn get_api_key(&self, key_hash: &str) -> Result<Option<ApiKey>>;
    async fn record_usage(&self, usage: &UsageRecord) -> Result<()>;
    // User, organization, provider, model management
}
```

### GatePlugin
```rust
#[async_trait]
pub trait GatePlugin: Send + Sync {
    fn name(&self) -> &str;
    async fn init(&mut self, context: PluginContext) -> Result<()>;
}
```

## Features

- `default`: Core functionality only
- `cloudflare`: Adds `worker` and `sqlx-d1` for Cloudflare Workers
- `tests`: Exposes test utilities and mocks

## Usage

Implement `StateBackend` for custom storage:
```rust
#[async_trait]
impl StateBackend for MyStorage {
    // Required method implementations
}
```

Create plugins by implementing `GatePlugin` and registering hooks during `init()`.

## Dependencies

Minimal set:
- `async-trait`, `thiserror` (required for trait system)
- `serde`, `chrono`, `http` (data types)
- `tokio` (sync primitives only, no runtime)

## Risks

- **WASM compatibility**: All code must work in both native and WASM targets
- **No panics**: Never use `unwrap()` or `expect()` - this is library code
- **Trait changes**: Breaking changes here affect entire ecosystem