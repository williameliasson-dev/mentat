use std::sync::Arc;

use object_store::ObjectStore;

pub mod account_repository;
pub mod vault_repository;
pub use account_repository::AccountRepository;
pub use vault_repository::{LogEntry, VaultRepository};

#[derive(Clone)]
pub struct Repositories {
    pub accounts: AccountRepository,
    pub vaults: VaultRepository,
}

impl Repositories {
    pub(crate) fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self {
            accounts: AccountRepository::new(store.clone()),
            vaults: VaultRepository::new(store),
        }
    }
}
