//! Exercises the auth layer over a real connection: the public surface must
//! stay reachable without a key, and the authenticated surface must reject
//! everything but a valid key paired with the caller's own account.
//!
//! Needs `DATABASE_URL`, i.e. a `devenv up` postgres.

use std::net::SocketAddr;

use proto::pb::account::v1::account_registry_client::AccountRegistryClient;
use proto::pb::account::v1::account_registry_server::AccountRegistryServer;
use proto::pb::account::v1::accounts_client::AccountsClient;
use proto::pb::account::v1::accounts_server::AccountsServer;
use proto::pb::account::v1::{CreateAccountRequest, GetAccountRequest};
use sync::grpc::{AccountRegistryRpc, AccountsRpc, AuthLayer};
use sync::{Database, Services};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::Code;
use tower::Layer;

/// Serves on an ephemeral port. The listener is bound before the server task
/// is spawned, so a client may connect immediately.
async fn serve() -> (SocketAddr, sqlx::PgPool) {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let db = Database::connect(&database_url).await.expect("connect");
    let pool = db.pool.clone();
    let services = Services::new(db.repositories());
    let auth = AuthLayer::new(services.accounts.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(AccountRegistryServer::new(AccountRegistryRpc::new(
                services.accounts.clone(),
            )))
            .add_service(auth.layer(AccountsServer::new(AccountsRpc::new())))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    (addr, pool)
}

fn with_key<T>(message: T, key: &str) -> tonic::Request<T> {
    let mut request = tonic::Request::new(message);
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {key}").parse().unwrap());
    request
}

#[tokio::test]
async fn auth_layer_guards_the_authenticated_surface() {
    let (addr, pool) = serve().await;
    let url = format!("http://{addr}");

    // Public: minting an account cannot require a key, since this is where
    // keys come from.
    let mut registry = AccountRegistryClient::connect(url.clone()).await.unwrap();
    let created = registry
        .create_account(CreateAccountRequest {})
        .await
        .expect("create_account is public")
        .into_inner();
    let account = created.account.expect("account");
    let key = created.account_key;

    let mut accounts = AccountsClient::connect(url).await.unwrap();

    // No metadata at all.
    let status = accounts
        .get_account(GetAccountRequest { id: account.id.clone() })
        .await
        .expect_err("no key must be rejected");
    assert_eq!(status.code(), Code::Unauthenticated);

    // A key that is not in the database.
    let status = accounts
        .get_account(with_key(
            GetAccountRequest { id: account.id.clone() },
            "notarealkey",
        ))
        .await
        .expect_err("an unknown key must be rejected");
    assert_eq!(status.code(), Code::Unauthenticated);

    // A valid key, but someone else's account: authenticated, not authorized.
    let status = accounts
        .get_account(with_key(
            GetAccountRequest { id: uuid::Uuid::new_v4().to_string() },
            &key,
        ))
        .await
        .expect_err("another account must be denied");
    assert_eq!(status.code(), Code::PermissionDenied);

    // A valid key and the caller's own account.
    let fetched = accounts
        .get_account(with_key(
            GetAccountRequest { id: account.id.clone() },
            &key,
        ))
        .await
        .expect("own account is readable")
        .into_inner()
        .account
        .expect("account");
    assert_eq!(fetched.id, account.id);
    assert_eq!(fetched.created_at, account.created_at);

    // This mints a real row; don't leave it behind.
    sqlx::query("DELETE FROM accounts WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&account.id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
}
