//! PostgreSQL-specific implementation

use crate::base::SqlxStateBackend;

/// PostgreSQL implementation of StateBackend
pub type PostgresStateBackend = SqlxStateBackend<sqlx::Postgres>;

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;
    use gate_core::tests::state::StateBackendTestSuite;

    async fn setup_postgres_backend() -> PostgresStateBackend {
        // This test requires DATABASE_URL to be set to a PostgreSQL instance
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/gate_test".to_string());

        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect(&database_url)
            .await
            .unwrap();

        let backend = PostgresStateBackend::from_pool(pool);

        // Clean up any existing data
        let _ = sqlx::query("DROP TABLE IF EXISTS usage_records, api_keys, users, models, providers, organizations CASCADE")
            .execute(backend.pool())
            .await;

        // Run migrations
        sqlx::query(include_str!("../migrations/0001_initial_schema.sql"))
            .execute(backend.pool())
            .await
            .unwrap();

        backend
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL to be running
    async fn test_postgres_compliance() {
        let backend = setup_postgres_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .run_all_tests()
            .await
            .expect("PostgreSQL backend should pass all tests");
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL to be running
    async fn test_postgres_user_operations() {
        let backend = setup_postgres_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_user_operations()
            .await
            .expect("User operations should work");
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL to be running
    async fn test_postgres_api_keys() {
        let backend = setup_postgres_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_api_key_operations()
            .await
            .expect("API key operations should work");
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL to be running
    async fn test_postgres_usage_tracking() {
        let backend = setup_postgres_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_usage_tracking()
            .await
            .expect("Usage tracking should work");
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL to be running
    async fn test_postgres_providers() {
        let backend = setup_postgres_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_provider_operations()
            .await
            .expect("Provider operations should work");
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL to be running
    async fn test_postgres_models() {
        let backend = setup_postgres_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_model_operations()
            .await
            .expect("Model operations should work");
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL to be running
    async fn test_postgres_organizations() {
        let backend = setup_postgres_backend().await;
        let suite = StateBackendTestSuite::new(backend);
        suite
            .test_organization_operations()
            .await
            .expect("Organization operations should work");
    }
}
