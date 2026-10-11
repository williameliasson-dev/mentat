use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub key_hash: String,
    pub created_at: DateTime<Utc>,
}
