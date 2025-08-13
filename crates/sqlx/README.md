# gate-sqlx

SQLite database implementation of Gate's `StateBackend` trait using SQLx. Provides production-ready storage for local deployments and Cloudflare D1.

## Responsibilities

- **StateBackend Implementation**: Full implementation for user, API key, usage tracking, and permissions
- **WebAuthn Storage**: Credential storage for hardware authentication
- **Database Migrations**: Schema management via SQLx migrate
- **Connection Pooling**: Efficient database connection management
- **Type-safe Queries**: Compile-time checked SQL via SQLx macros

## Organization

```
src/
├── sqlite.rs    # SQLite implementation of StateBackend
├── webauthn.rs  # WebAuthn credential storage
└── common.rs    # Shared types and helpers

migrations/
├── 0001_initial_schema.sql  # Core tables
├── 0002_webauthn_schema.sql # WebAuthn tables
└── 0003_permissions_schema.sql # RBAC permissions
```

## Features

- `default`: SQLite support
- `sqlite`: SQLite support

## Usage

```rust
use gate_sqlx::SqliteStateBackend;

let backend = SqliteStateBackend::new("sqlite://gate.db").await?;
// Migrations run automatically in new()
```

## Database Schema

### Core Tables
- `users`: User accounts with metadata
- `organizations`: Organization/tenant data
- `api_keys`: Hashed API keys with configuration
- `usage_records`: Request usage tracking
- `providers`: AI provider configurations
- `models`: Available AI models

### WebAuthn Tables
- `webauthn_credentials`: Hardware authentication credentials

### Permission Tables
- `permissions`: Subject-action-object permission tuples for RBAC

## Dependencies

- `sqlx`: Database driver with compile-time checked queries
- `gate-core`: StateBackend trait and types
- Runtime: `tokio` with rustls

## Migrations

Migrations are embedded in the binary and run automatically when creating a new backend:
```rust
let backend = SqliteStateBackend::new("sqlite://gate.db").await?;
// Migrations already applied
```

## Deployment Targets

- **Local/Daemon**: Native SQLite file database
- **Cloudflare Workers**: D1 (SQLite-compatible cloud database)

## Risks

- **Schema Changes**: Migrations must be backwards compatible
- **SQLx Offline Mode**: Requires database for compilation unless using offline mode