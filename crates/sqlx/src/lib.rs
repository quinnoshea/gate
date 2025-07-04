//! SQLx-based StateBackend implementations for PostgreSQL and SQLite

mod base;
mod common;
mod webauthn;

#[cfg(feature = "postgres")]
mod postgres;

#[cfg(feature = "sqlite")]
mod sqlite;

// Re-export the base type for those who need the generic version
pub use base::SqlxStateBackend;

// Re-export WebAuthn backend
pub use webauthn::SqlxWebAuthnBackend;

// Re-export database-specific types
#[cfg(feature = "postgres")]
pub use postgres::PostgresStateBackend;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStateBackend;
