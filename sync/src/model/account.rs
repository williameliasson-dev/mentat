use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stored at `accounts/<key hash>.json`. The key hash is the object's name
/// rather than a field, since that is the only thing accounts are looked up
/// by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
}
