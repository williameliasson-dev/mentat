use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::error::{Result, SyncError};
use crate::model::Account;
use crate::repository::AccountRepository;

/// How long a resolved token is trusted without asking the bucket again. Every
/// authenticated rpc resolves its token, and a bucket read is far slower than
/// the rpc itself. Accounts never change once created, so the only staleness
/// is a deleted account's token working for up to this long.
const CACHE_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct AccountService {
    repository: AccountRepository,
    /// Token hash to account, for tokens that resolved. Unknown tokens are never
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

    /// Registers `auth_token` as a new account. Registering a token that is
    /// already registered returns its account rather than failing: only the
    /// token's holder can do that, and it lets a client retry a signup whose
    /// response it never got.
    pub async fn register(&self, auth_token: &str) -> Result<Account> {
        // The client derives tokens from a random key, so anything else is a
        // bug or a guess, and a short guess would be easy to collide with.
        if auth_token.len() != 64
            || !auth_token
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(SyncError::InvalidAuthToken);
        }
        let key_hash = hash_auth_token(auth_token);
        match self.repository.create(&key_hash).await {
            Err(SyncError::Conflict) => self.repository.find_by_key_hash(&key_hash).await,
            result => result,
        }
    }

    pub async fn authenticate(&self, auth_token: &str) -> Result<Account> {
        let key_hash = hash_auth_token(auth_token);
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

/// Tokens are 256-bit HKDF outputs, so a plain hash is enough to keep them
/// out of the bucket: there is nothing to brute-force, unlike a password.
fn hash_auth_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
