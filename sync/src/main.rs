use std::net::SocketAddr;

use sync::Services;
use sync::app::app;
use sync::database::Database;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = Database::connect(&database_url)
        .await
        .expect("failed to connect to database");
    let services = Services::new(db.repositories());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app(services)).await.unwrap();
}
