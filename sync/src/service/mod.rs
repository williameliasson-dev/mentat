use crate::repository::Repositories;

pub mod account_service;
pub mod vault_service;
pub use account_service::AccountService;
pub use vault_service::VaultService;

#[derive(Clone)]
pub struct Services {
    pub accounts: AccountService,
    pub vaults: VaultService,
}

impl Services {
    pub fn new(repositories: Repositories) -> Self {
        Self {
            accounts: AccountService::new(repositories.accounts),
            vaults: VaultService::new(repositories.vaults),
        }
    }
}
