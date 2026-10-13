//! Serves the real server over a real connection, against the `S3_*` bucket
//! (i.e. a `devenv up s3` versitygw), confined to a fresh prefix per test.

use std::net::SocketAddr;
use std::sync::Arc;

use futures::TryStreamExt;
use object_store::{ObjectStore, ObjectStoreExt};
use proto::pb::account::v1::CreateAccountRequest;
use proto::pb::account::v1::account_registry_client::AccountRegistryClient;
use proto::pb::account::v1::account_registry_server::AccountRegistryServer;
use proto::pb::account::v1::accounts_server::AccountsServer;
use proto::pb::vault::v1::vaults_server::VaultsServer;
use sync::grpc::{AccountRegistryRpc, AccountsRpc, AuthLayer, VaultsRpc};
use sync::{Services, Storage};
use tokio_stream::wrappers::TcpListenerStream;
use tower::Layer;

pub struct Server {
    pub url: String,
    store: Arc<dyn ObjectStore>,
}

impl Server {
    /// Serves on an ephemeral port. The listener is bound before the server
    /// task is spawned, so a client may connect immediately.
    pub async fn start() -> Self {
        let prefix = format!("test-{}", uuid::Uuid::new_v4());
        let storage = Storage::from_env().expect("S3_* env").with_prefix(&prefix);
        let store = storage.store.clone();
        let services = Services::new(storage.repositories());
        let auth = AuthLayer::new(services.accounts.clone());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(AccountRegistryServer::new(AccountRegistryRpc::new(
                    services.accounts.clone(),
                )))
                .add_service(auth.layer(AccountsServer::new(AccountsRpc::new())))
                .add_service(auth.layer(VaultsServer::new(VaultsRpc::new(services.vaults.clone()))))
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        Self {
            url: format!("http://{addr}"),
            store,
        }
    }

    /// Mints an account and returns its id and key.
    pub async fn create_account(&self) -> (String, String) {
        let created = AccountRegistryClient::connect(self.url.clone())
            .await
            .unwrap()
            .create_account(CreateAccountRequest {})
            .await
            .expect("create_account is public")
            .into_inner();
        (created.account.expect("account").id, created.account_key)
    }

    /// Deletes everything this test wrote, which is everything under its
    /// prefix.
    pub async fn cleanup(self) {
        let paths: Vec<_> = self
            .store
            .list(None)
            .map_ok(|meta| meta.location)
            .try_collect()
            .await
            .unwrap();
        for path in paths {
            self.store.delete(&path).await.unwrap();
        }
    }
}

pub fn with_key<T>(message: T, key: &str) -> tonic::Request<T> {
    let mut request = tonic::Request::new(message);
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {key}").parse().unwrap());
    request
}
