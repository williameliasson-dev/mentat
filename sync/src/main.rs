use std::net::SocketAddr;

use sync::grpc::AccountsRpc;
use sync::pb::account::v1::accounts_server::AccountsServer;
use sync::service::Services;
use sync::Database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;
    let db = Database::connect(&database_url).await?;
    let services = Services::new(db.repositories());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {addr}");

    tonic::transport::Server::builder()
        .add_service(AccountsServer::new(AccountsRpc::new(services)))
        .serve(addr)
        .await?;
    Ok(())
}
