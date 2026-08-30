use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct Account {
    pub id: Uuid,
    #[serde(skip_serializing)]
    pub account_key: String,
    pub created_at: DateTime<Utc>,
}
