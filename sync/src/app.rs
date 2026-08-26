use std::sync::Arc;

use axum::Router;

use crate::Services;

pub fn app(services: Services) -> Router {
    Router::new().with_state(Arc::new(services))
}
