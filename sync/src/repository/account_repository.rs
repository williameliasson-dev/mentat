use std::sync::Arc;

use chrono::Utc;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutMode};
use uuid::Uuid;

use crate::error::Result;
use crate::model::Account;

#[derive(Clone)]
pub struct AccountRepository {
    store: Arc<dyn ObjectStore>,
}

impl AccountRepository {
    pub(crate) fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }

    pub async fn create(&self, key_hash: &str) -> Result<Account> {
        let account = Account {
            id: Uuid::new_v4(),
            created_at: Utc::now(),
        };
        // Create, never overwrite: an existing object here means two keys
        // hashed the same, and overwriting would hand one account to the
        // other's key.
        self.store
            .put_opts(
                &path(key_hash),
                serde_json::to_vec(&account)?.into(),
                PutMode::Create.into(),
            )
            .await?;
        Ok(account)
    }

    pub async fn find_by_key_hash(&self, key_hash: &str) -> Result<Account> {
        let bytes = self.store.get(&path(key_hash)).await?.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn path(key_hash: &str) -> Path {
    Path::from_iter(["accounts", &format!("{key_hash}.json")])
}
