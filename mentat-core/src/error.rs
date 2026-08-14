use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("note not found: {0}")]
    NotFound(i64),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
