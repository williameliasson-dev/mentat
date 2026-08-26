use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::model::Account;
use crate::error::Result;
use crate::repository::AccountRepository;

pub struct AccountService {
    repository: AccountRepository,
}

impl AccountService {
    pub fn new(repository: AccountRepository) -> Self {
        Self { repository }
    }

    pub async fn register(&self) -> Result<(Account, String)> {
        let plaintext_key = generate_account_key();
        let account = self
            .repository
            .create(&hash_account_key(&plaintext_key))
            .await?;
        Ok((account, plaintext_key))
    }

    pub async fn get(&self, id: Uuid) -> Result<Account> {
        self.repository.find_by_id(id).await
    }

    pub async fn authenticate(&self, account_key: &str) -> Result<Account> {
        self.repository
            .find_by_account_key(&hash_account_key(account_key))
            .await
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
