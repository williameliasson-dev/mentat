use std::net::SocketAddr;

use proto::pb::account::v1::account_registry_server::AccountRegistryServer;
use proto::pb::account::v1::accounts_server::AccountsServer;
use sync::Database;
use sync::grpc::{AccountRegistryRpc, AccountsRpc, AuthLayer};
use sync::service::Services;
use tower::Layer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;
    let db = Database::connect(&database_url).await?;

    let services = Services::new(db.repositories());

    // Mounting a server through `auth` is what makes its rpcs require a key,
    // so the public and authenticated surfaces stay visibly separate here.
    let auth = AuthLayer::new(services.accounts.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {addr}");

    tonic::transport::Server::builder()
        .add_service(AccountRegistryServer::new(AccountRegistryRpc::new(
            services.accounts.clone(),
        )))
        .add_service(auth.layer(AccountsServer::new(AccountsRpc::new())))
        .serve(addr)
        .await?;
    Ok(())
}
