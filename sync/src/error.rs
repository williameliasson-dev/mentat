use thiserror::Error;
use tonic::Status;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("not found")]
    NotFound,

    #[error("an auth token is 64 lowercase hex characters")]
    InvalidAuthToken,

    /// A conditional write lost: the object already existed, or changed since
    /// the caller last read it.
    #[error("conflict")]
    Conflict,

    #[error("storage error")]
    Storage(object_store::Error),

    #[error("encoding error")]
    Encoding(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SyncError>;

// The bucket's own outcomes become ours, so repositories can just use `?`.
impl From<object_store::Error> for SyncError {
    fn from(error: object_store::Error) -> Self {
        match error {
            object_store::Error::NotFound { .. } => SyncError::NotFound,
            object_store::Error::AlreadyExists { .. }
            | object_store::Error::Precondition { .. } => SyncError::Conflict,
            error => SyncError::Storage(error),
        }
    }
}

impl From<SyncError> for Status {
    fn from(error: SyncError) -> Self {
        match &error {
            SyncError::NotFound => Status::not_found(error.to_string()),
            SyncError::InvalidAuthToken => Status::invalid_argument(error.to_string()),
            SyncError::Conflict => Status::aborted(error.to_string()),
            SyncError::Storage(_) | SyncError::Encoding(_) => Status::internal("internal error"),
        }
    }
}
