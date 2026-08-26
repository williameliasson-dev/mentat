use axum::Router;
use sqlx::PgPool;

pub fn app(pool: PgPool) -> Router {
    Router::new().with_state(pool)
}
