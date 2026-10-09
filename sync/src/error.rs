use tonic::Status;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("account not found")]
    NotFound,

    #[error("database error")]
    Database(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, SyncError>;

impl From<SyncError> for Status {
    fn from(error: SyncError) -> Self {
        match &error {
            SyncError::NotFound => Status::not_found(error.to_string()),
            SyncError::Database(_) => Status::internal("internal error"),
        }
    }
}
