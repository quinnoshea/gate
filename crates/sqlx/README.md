# gate-sqlx

Native SQL database implementations of Gate's `StateBackend` trait using SQLx. Provides production-ready storage for PostgreSQL and SQLite.

## Responsibilities

- **StateBackend Implementation**: Full implementation for user, API key, and usage tracking
- **WebAuthn Storage**: Credential storage for hardware authentication
- **Database Migrations**: Schema management via SQLx migrate
- **Connection Pooling**: Efficient database connection management
- **Type-safe Queries**: Compile-time checked SQL via SQLx macros

## Organization

```
src/
├── base.rs      # Generic SqlxStateBackend<DB>
├── postgres.rs  # PostgreSQL-specific implementation
├── sqlite.rs    # SQLite-specific implementation
├── webauthn.rs  # WebAuthn credential storage
└── common.rs    # Shared SQL queries and helpers

migrations/
├── 0001_initial_schema.sql  # Core tables
└── 0002_webauthn_schema.sql # WebAuthn tables
```

## Features

- `default`: Both PostgreSQL and SQLite support
- `postgres`: PostgreSQL support only
- `sqlite`: SQLite support only

## Usage

### PostgreSQL
```rust
use gate_sqlx::PostgresStateBackend;

let backend = PostgresStateBackend::new("postgres://user:pass@localhost/gate").await?;
backend.run_migrations().await?;
```

### SQLite
```rust
use gate_sqlx::SqliteStateBackend;

let backend = SqliteStateBackend::new("sqlite://gate.db").await?;
backend.run_migrations().await?;
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

## Dependencies

- `sqlx`: Database driver with compile-time checked queries
- `gate-core`: StateBackend trait and types
- Runtime: `tokio` with rustls

## Migrations

Run migrations before first use:
```rust
backend.run_migrations().await?;
```

Migrations are embedded in the binary and run automatically.

## Risks

- **Schema Changes**: Migrations must be backwards compatible
- **SQLx Offline Mode**: Requires database for compilation unless using offline mode
- **Platform Differences**: Some SQL may need per-database variants