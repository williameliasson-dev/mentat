use std::net::SocketAddr;

use proto::pb::account::v1::account_registry_server::AccountRegistryServer;
use proto::pb::account::v1::accounts_server::AccountsServer;
use proto::pb::vault::v1::vaults_server::VaultsServer;
use sync::Storage;
use sync::grpc::{AccountRegistryRpc, AccountsRpc, AuthLayer, VaultsRpc};
use sync::service::Services;
use tower::Layer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = Storage::from_env()?;
    let services = Services::new(storage.repositories());

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
        .add_service(auth.layer(VaultsServer::new(VaultsRpc::new(services.vaults.clone()))))
        .serve(addr)
        .await?;
    Ok(())
}
