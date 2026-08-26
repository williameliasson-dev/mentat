use sqlx::PgPool;
use uuid::Uuid;

use crate::model::Account;
use crate::error::{Result, SyncError};

pub struct AccountRepository {
    pool: PgPool,
}

impl AccountRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, account_key: &str) -> Result<Account> {
        Ok(sqlx::query_as::<_, Account>(
            "INSERT INTO accounts (account_key) VALUES ($1)
             RETURNING id, account_key, created_at",
        )
        .bind(account_key)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT id, account_key, created_at FROM accounts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SyncError::NotFound)
    }

    pub async fn find_by_account_key(&self, account_key: &str) -> Result<Account> {
        sqlx::query_as::<_, Account>(
            "SELECT id, account_key, created_at FROM accounts WHERE account_key = $1",
        )
        .bind(account_key)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SyncError::NotFound)
    }
}
