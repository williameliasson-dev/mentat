use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("note not found: {0}")]
    NotFound(i64),

    /// Moving a folder into itself or into one of its own descendants, which
    /// would cut the subtree loose from the root.
    #[error("cannot move a folder into itself")]
    InvalidMove,

    /// A sibling already uses that name — folders are unique per parent.
    #[error("\"{0}\" already exists there")]
    NameTaken(String),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
