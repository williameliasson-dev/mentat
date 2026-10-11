use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Result, SyncError};
use crate::model::Account;

#[derive(Clone)]
pub struct AccountRepository {
    pool: PgPool,
}

impl AccountRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, key_hash: &str) -> Result<Account> {
        Ok(sqlx::query_as::<_, Account>(
            "INSERT INTO accounts (key_hash) VALUES ($1)
             RETURNING id, key_hash, created_at",
        )
        .bind(key_hash)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT id, key_hash, created_at FROM accounts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SyncError::NotFound)
    }

    pub async fn find_by_key_hash(&self, key_hash: &str) -> Result<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT id, key_hash, created_at FROM accounts WHERE key_hash = $1",
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SyncError::NotFound)
    }
}
