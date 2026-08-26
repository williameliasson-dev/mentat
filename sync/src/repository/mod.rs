use sqlx::PgPool;

pub mod account_repository;
pub use account_repository::AccountRepository;

pub struct Repositories {
    pub accounts: AccountRepository,
}

impl Repositories {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self {
            accounts: AccountRepository::new(pool),
        }
    }
}
