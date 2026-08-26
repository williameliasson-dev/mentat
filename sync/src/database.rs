use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::repository::Repositories;

pub struct Database {
    pub pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    /// Every repository, each holding a handle to this same pool.
    pub fn repositories(&self) -> Repositories {
        Repositories::new(self.pool.clone())
    }
}
