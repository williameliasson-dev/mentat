use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("account not found")]
    NotFound,

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("invalid request body: {0}")]
    InvalidJson(#[from] JsonRejection),

    #[error("validation failed: {0}")]
    Validation(#[from] garde::Report),
}

pub type Result<T> = std::result::Result<T, SyncError>;

impl IntoResponse for SyncError {
    fn into_response(self) -> Response {
        let status = match &self {
            SyncError::NotFound => StatusCode::NOT_FOUND,
            SyncError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SyncError::InvalidJson(_) | SyncError::Validation(_) => StatusCode::BAD_REQUEST,
        };
        (status, self.to_string()).into_response()
    }
}
