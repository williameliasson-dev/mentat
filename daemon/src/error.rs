use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("not found: {0}")]
    NotFound(Uuid),

    /// Moving a folder into itself or into one of its own descendants, which
    /// would cut the subtree loose from the root.
    #[error("cannot move a folder into itself")]
    InvalidMove,

    /// A sibling already uses that name — folders are unique per parent.
    #[error("\"{0}\" already exists there")]
    NameTaken(String),

    #[error("storage error: {0}")]
    Io(#[from] std::io::Error),

    #[error("document error: {0}")]
    Loro(#[from] loro::LoroError),

    #[error("document encoding error: {0}")]
    Encode(#[from] loro::LoroEncodeError),

    #[error("sync: {0}")]
    Sync(String),

    #[error("sync server: {}", .0.message())]
    Remote(#[from] tonic::Status),

    #[error("could not reach the sync server: {0}")]
    Connect(#[from] tonic::transport::Error),

    #[error("sync: {0}")]
    Crypto(#[from] crypto::CryptoError),

    #[error("sync state: {0}")]
    State(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
