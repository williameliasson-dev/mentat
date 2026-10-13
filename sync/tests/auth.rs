//! Exercises the auth layer over a real connection: the public surface must
//! stay reachable without a key, and the authenticated surface must reject
//! everything but a valid key paired with the caller's own account.
//!
//! Needs the `S3_*` bucket, i.e. a `devenv up s3` versitygw.

mod common;

use common::{Server, with_key};
use proto::pb::account::v1::GetAccountRequest;
use proto::pb::account::v1::accounts_client::AccountsClient;
use tonic::Code;

#[tokio::test]
async fn auth_layer_guards_the_authenticated_surface() {
    let server = Server::start().await;

    // Public: minting an account cannot require a key, since this is where
    // keys come from.
    let (account_id, key) = server.create_account().await;

    let mut accounts = AccountsClient::connect(server.url.clone()).await.unwrap();

    // No metadata at all.
    let status = accounts
        .get_account(GetAccountRequest {
            id: account_id.clone(),
        })
        .await
        .expect_err("no key must be rejected");
    assert_eq!(status.code(), Code::Unauthenticated);

    // A key that no account was created with.
    let status = accounts
        .get_account(with_key(
            GetAccountRequest {
                id: account_id.clone(),
            },
            "notarealkey",
        ))
        .await
        .expect_err("an unknown key must be rejected");
    assert_eq!(status.code(), Code::Unauthenticated);

    // A valid key, but someone else's account: authenticated, not authorized.
    let status = accounts
        .get_account(with_key(
            GetAccountRequest {
                id: uuid::Uuid::new_v4().to_string(),
            },
            &key,
        ))
        .await
        .expect_err("another account must be denied");
    assert_eq!(status.code(), Code::PermissionDenied);

    // A valid key and the caller's own account.
    let fetched = accounts
        .get_account(with_key(
            GetAccountRequest {
                id: account_id.clone(),
            },
            &key,
        ))
        .await
        .expect("own account is readable")
        .into_inner()
        .account
        .expect("account");
    assert_eq!(fetched.id, account_id);

    server.cleanup().await;
}
