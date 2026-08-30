use std::sync::Arc;

use axum::{Router, routing::post};

use crate::{Services, routes::account::create_account};

pub fn app(services: Services) -> Router {
    Router::new()
        .with_state(Arc::new(services))
        .route("account", post(create_account))
}
