use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::Result;
use crate::model::Account;
use crate::repository::AccountRepository;

/// How long a resolved key is trusted without asking the bucket again. Every
/// authenticated rpc resolves its key, and a bucket read is far slower than
/// the rpc itself. Accounts never change once created, so the only staleness
/// is a deleted account's key working for up to this long.
const CACHE_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct AccountService {
    repository: AccountRepository,
    /// Key hash to account, for keys that resolved. Unknown keys are never
    /// cached, so this only grows with real accounts.
    cache: Arc<RwLock<HashMap<String, (Account, Instant)>>>,
}

impl AccountService {
    pub fn new(repository: AccountRepository) -> Self {
        Self {
            repository,
            cache: Arc::default(),
        }
    }

    pub async fn register(&self) -> Result<(Account, String)> {
        let plaintext_key = generate_account_key();
        let account = self
            .repository
            .create(&hash_account_key(&plaintext_key))
            .await?;
        Ok((account, plaintext_key))
    }

    pub async fn authenticate(&self, account_key: &str) -> Result<Account> {
        let key_hash = hash_account_key(account_key);
        if let Some((account, cached_at)) = self.cache.read().unwrap().get(&key_hash)
            && cached_at.elapsed() < CACHE_TTL
        {
            return Ok(account.clone());
        }

        let account = self.repository.find_by_key_hash(&key_hash).await?;
        self.cache
            .write()
            .unwrap()
            .insert(key_hash, (account.clone(), Instant::now()));
        Ok(account)
    }
}

fn generate_account_key() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn hash_account_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}
