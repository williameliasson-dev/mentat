use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("account not found")]
    NotFound,

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, SyncError>;
