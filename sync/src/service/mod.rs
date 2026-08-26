use crate::repository::Repositories;

pub mod account_service;
pub use account_service::AccountService;

pub struct Services {
    pub accounts: AccountService,
}

impl Services {
    pub fn new(repositories: Repositories) -> Self {
        Self {
            accounts: AccountService::new(repositories.accounts),
        }
    }
}
