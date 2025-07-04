//! Base generic SQLx implementation

use async_trait::async_trait;
use gate_core::{
    ApiKey, Error, Model, Organization, Provider, Result, StateBackend, TimeRange, UsageRecord,
    User,
};
use sqlx::{Database, Executor, FromRow, IntoArguments, Pool};
use std::marker::PhantomData;
use tracing::instrument;

use crate::common::*;

/// Generic SQLx implementation of StateBackend
pub struct SqlxStateBackend<DB: Database> {
    pool: Pool<DB>,
    _phantom: PhantomData<DB>,
}

impl<DB: Database> SqlxStateBackend<DB> {
    pub fn from_pool(pool: Pool<DB>) -> Self {
        Self {
            pool,
            _phantom: PhantomData,
        }
    }

    /// Get the underlying pool (for running migrations externally)
    pub fn pool(&self) -> &Pool<DB> {
        &self.pool
    }

    /// Get connection pool metrics
    pub fn pool_metrics(&self) -> (usize, usize, usize) {
        let pool_size = self.pool.size() as usize;
        let idle_count = self.pool.num_idle();
        let active_count = pool_size.saturating_sub(idle_count);

        (active_count, idle_count, pool_size)
    }
}

#[async_trait]
impl<DB> StateBackend for SqlxStateBackend<DB>
where
    DB: Database,
    for<'c> &'c mut <DB as Database>::Connection: Executor<'c, Database = DB>,
    for<'r> UserRow: FromRow<'r, DB::Row>,
    for<'r> ApiKeyRow: FromRow<'r, DB::Row>,
    for<'r> UsageRecordRow: FromRow<'r, DB::Row>,
    for<'r> ProviderRow: FromRow<'r, DB::Row>,
    for<'r> ModelRow: FromRow<'r, DB::Row>,
    for<'r> OrganizationRow: FromRow<'r, DB::Row>,
    // Required for async_trait with generic parameters
    DB: Send + Sync,
    DB::Connection: Send,
    // Required for parameter binding
    for<'q> String: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> &'q str: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> Option<String>: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> i32: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    for<'q> f64: sqlx::Encode<'q, DB> + sqlx::Type<DB>,
    // Required for queries
    for<'q> <DB as Database>::Arguments<'q>: IntoArguments<'q, DB>,
{
    // User management
    #[instrument(name = "db.get_user", skip(self))]
    async fn get_user(&self, user_id: &str) -> Result<Option<User>> {
        // Just delegate to get_user_by_id for consistency
        self.get_user_by_id(user_id).await
    }

    #[instrument(name = "db.get_user_by_id", skip(self))]
    async fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, name, role, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Database error: {e}")))?;

        Ok(user.map(Into::into))
    }

    #[instrument(name = "db.create_user", skip(self, user), fields(user_id = %user.id))]
    async fn create_user(&self, user: &User) -> Result<()> {
        let email = user.metadata.get("email").cloned();

        sqlx::query(
            "INSERT INTO users (id, email, name, role, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(&user.id)
        .bind(email)
        .bind(&user.name)
        .bind(&user.role)
        .bind(datetime_to_string(user.created_at))
        .bind(datetime_to_string(user.updated_at))
        .execute(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Failed to create user: {e}")))?;

        Ok(())
    }

    async fn update_user(&self, user: &User) -> Result<()> {
        let email = user.metadata.get("email").cloned();

        sqlx::query(
            "UPDATE users SET email = $1, name = $2, role = $3, updated_at = $4 WHERE id = $5",
        )
        .bind(email)
        .bind(&user.name)
        .bind(&user.role)
        .bind(datetime_to_string(user.updated_at))
        .bind(&user.id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Failed to update user: {e}")))?;

        Ok(())
    }

    #[instrument(name = "db.delete_user", skip(self))]
    async fn delete_user(&self, user_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::StateError(format!("Failed to delete user: {e}")))?;

        Ok(())
    }

    #[instrument(name = "db.list_users", skip(self))]
    async fn list_users(&self, filter: Option<&str>) -> Result<Vec<User>> {
        let users = if let Some(role_filter) = filter {
            sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, role, created_at, updated_at FROM users WHERE role = $1 ORDER BY created_at DESC",
            )
            .bind(role_filter)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, UserRow>(
                "SELECT id, email, name, role, created_at, updated_at FROM users ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| Error::StateError(format!("Failed to list users: {e}")))?;

        Ok(users.into_iter().map(Into::into).collect())
    }

    // API key management
    async fn get_api_key(&self, key_hash: &str) -> Result<Option<ApiKey>> {
        let key = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT key_hash, name, org_id, config, created_at, last_used_at FROM api_keys WHERE key_hash = $1"
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Database error: {e}")))?;

        Ok(key.map(Into::into))
    }

    async fn create_api_key(&self, key: &ApiKey, _raw_key: &str) -> Result<()> {
        let config_json = key
            .config
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap());

        sqlx::query(
            "INSERT INTO api_keys (key_hash, name, org_id, config, created_at, last_used_at) VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(&key.key_hash)
        .bind(&key.name)
        .bind(&key.org_id)
        .bind(config_json)
        .bind(datetime_to_string(key.created_at))
        .bind(key.last_used_at.map(datetime_to_string))
        .execute(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Failed to create API key: {e}")))?;

        Ok(())
    }

    async fn list_api_keys(&self, org_id: &str) -> Result<Vec<ApiKey>> {
        let keys = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT key_hash, name, org_id, config, created_at, last_used_at FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC"
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Failed to list API keys: {e}")))?;

        Ok(keys.into_iter().map(Into::into).collect())
    }

    async fn delete_api_key(&self, key_hash: &str) -> Result<()> {
        sqlx::query("DELETE FROM api_keys WHERE key_hash = $1")
            .bind(key_hash)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::StateError(format!("Failed to delete API key: {e}")))?;

        Ok(())
    }

    // Usage tracking
    async fn record_usage(&self, usage: &UsageRecord) -> Result<()> {
        let metadata_json = if usage.metadata.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&usage.metadata).unwrap_or_default())
        };

        sqlx::query(
            "INSERT INTO usage_records (id, org_id, user_id, api_key_hash, request_id, provider_id, model_id, input_tokens, output_tokens, total_tokens, cost, timestamp, metadata) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"
        )
        .bind(&usage.id)
        .bind(&usage.org_id)
        .bind(&usage.user_id)
        .bind(&usage.api_key_hash)
        .bind(&usage.request_id)
        .bind(&usage.provider_id)
        .bind(&usage.model_id)
        .bind(usage.input_tokens as i64)
        .bind(usage.output_tokens as i64)
        .bind(usage.total_tokens as i64)
        .bind(usage.cost)
        .bind(datetime_to_string(usage.timestamp))
        .bind(metadata_json)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Failed to record usage: {e}")))?;

        Ok(())
    }

    async fn get_usage(&self, org_id: &str, range: &TimeRange) -> Result<Vec<UsageRecord>> {
        let records = sqlx::query_as::<_, UsageRecordRow>(
            "SELECT * FROM usage_records WHERE org_id = $1 AND timestamp >= $2 AND timestamp <= $3 ORDER BY timestamp DESC"
        )
        .bind(org_id)
        .bind(datetime_to_string(range.start))
        .bind(datetime_to_string(range.end))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Failed to get usage records: {e}")))?;

        Ok(records.into_iter().map(Into::into).collect())
    }

    // Provider management
    async fn get_provider(&self, id: &str) -> Result<Option<Provider>> {
        let provider = sqlx::query_as::<_, ProviderRow>(
            "SELECT id, name, provider_type, config, enabled, priority FROM providers WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Database error: {e}")))?;

        Ok(provider.map(Into::into))
    }

    async fn list_providers(&self) -> Result<Vec<Provider>> {
        let providers = sqlx::query_as::<_, ProviderRow>(
            "SELECT id, name, provider_type, config, enabled, priority FROM providers WHERE enabled = 1 ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Failed to list providers: {e}")))?;

        Ok(providers.into_iter().map(Into::into).collect())
    }

    // Model management
    async fn get_model(&self, id: &str) -> Result<Option<Model>> {
        let model = sqlx::query_as::<_, ModelRow>(
            "SELECT id, provider_id, name, model_type, capabilities, pricing_id, pricing_config FROM models WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Database error: {e}")))?;

        Ok(model.map(Into::into))
    }

    async fn list_models(&self) -> Result<Vec<Model>> {
        let models = sqlx::query_as::<_, ModelRow>(
            "SELECT id, provider_id, name, model_type, capabilities, pricing_id, pricing_config FROM models ORDER BY provider_id, name"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Failed to list models: {e}")))?;

        Ok(models.into_iter().map(Into::into).collect())
    }

    // Organization management
    async fn get_organization(&self, id: &str) -> Result<Option<Organization>> {
        let org = sqlx::query_as::<_, OrganizationRow>(
            "SELECT id, name, created_at, settings FROM organizations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Database error: {e}")))?;

        Ok(org.map(Into::into))
    }

    async fn create_organization(&self, org: &Organization) -> Result<()> {
        let settings_json = if org.settings.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&org.settings).unwrap_or_default())
        };

        sqlx::query(
            "INSERT INTO organizations (id, name, created_at, settings) VALUES ($1, $2, $3, $4)",
        )
        .bind(&org.id)
        .bind(&org.name)
        .bind(datetime_to_string(org.created_at))
        .bind(settings_json)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::StateError(format!("Failed to create organization: {e}")))?;

        Ok(())
    }
}
