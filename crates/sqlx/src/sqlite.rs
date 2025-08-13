//! SQLite-specific implementation

use crate::base::SqlxStateBackend;
use gate_core::Result;

/// SQLite implementation of StateBackend
pub type SqliteStateBackend = SqlxStateBackend<sqlx::Sqlite>;

impl SqliteStateBackend {
    /// Create a new SQLite backend and run migrations
    pub async fn new(database_url: &str) -> Result<Self> {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;

        // Parse the URL and ensure create_if_missing is enabled
        let options = SqliteConnectOptions::from_str(database_url)
            .map_err(|e| gate_core::Error::StateError(format!("Invalid database URL: {e}")))?
            .create_if_missing(true);

        let pool = sqlx::SqlitePool::connect_with(options).await.map_err(|e| {
            gate_core::Error::StateError(format!("Failed to connect to database: {e}"))
        })?;

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| gate_core::Error::StateError(format!("Failed to run migrations: {e}")))?;

        Ok(Self::from_pool(pool))
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use gate_core::tests::state::StateBackendTestSuite;

    async fn setup_sqlite_backend() -> SqliteStateBackend {
        use sqlx::sqlite::SqlitePoolOptions;

        // Create in-memory SQLite database with proper configuration
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        // Run migrations in order
        sqlx::query(include_str!("../migrations/0001_initial_schema.sql"))
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(include_str!("../migrations/0002_webauthn_schema.sql"))
            .execute(&pool)
            .await
            .unwrap();

        SqliteStateBackend::from_pool(pool)
    }

    #[tokio::test]
    async fn test_sqlite_compliance() {
        let backend = setup_sqlite_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .run_all_tests()
            .await
            .expect("SQLite backend should pass all tests");
    }

    #[tokio::test]
    async fn test_sqlite_user_operations() {
        let backend = setup_sqlite_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_user_operations()
            .await
            .expect("User operations should work");
    }

    #[tokio::test]
    async fn test_sqlite_api_keys() {
        let backend = setup_sqlite_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_api_key_operations()
            .await
            .expect("API key operations should work");
    }

    #[tokio::test]
    async fn test_sqlite_usage_tracking() {
        let backend = setup_sqlite_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_usage_tracking()
            .await
            .expect("Usage tracking should work");
    }

    #[tokio::test]
    async fn test_sqlite_providers() {
        let backend = setup_sqlite_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_provider_operations()
            .await
            .expect("Provider operations should work");
    }

    #[tokio::test]
    async fn test_sqlite_models() {
        let backend = setup_sqlite_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_model_operations()
            .await
            .expect("Model operations should work");
    }

    #[tokio::test]
    async fn test_sqlite_organizations() {
        let backend = setup_sqlite_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_organization_operations()
            .await
            .expect("Organization operations should work");
    }
}
