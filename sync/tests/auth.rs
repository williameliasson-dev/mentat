//! Exercises the auth layer over a real connection: the public surface must
//! stay reachable without a key, and the authenticated surface must reject
//! everything but a valid key paired with the caller's own account.
//!
//! Needs the `S3_*` bucket, i.e. a `devenv up s3` versitygw.

mod common;

use common::{Server, with_key};
use crypto::AccountKey;
use proto::pb::account::v1::account_registry_client::AccountRegistryClient;
use proto::pb::account::v1::accounts_client::AccountsClient;
use proto::pb::account::v1::{AuthenticateRequest, CreateAccountRequest, GetAccountRequest};
use tonic::Code;

#[tokio::test]
async fn auth_layer_guards_the_authenticated_surface() {
    let server = Server::start().await;

    // Public: registering cannot require a token, since this is where tokens
    // become valid.
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

    // A token no account was registered with.
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

#[tokio::test]
async fn registration_takes_only_client_derived_tokens() {
    let server = Server::start().await;
    let mut registry = AccountRegistryClient::connect(server.url.clone())
        .await
        .unwrap();
    let register = |auth_token: String| CreateAccountRequest { auth_token };

    // Anything that is not a derived token is refused, so a short guessable
    // string can never become an account's credential.
    for bad in ["", "abc123", &"A".repeat(64), &"0".repeat(63)] {
        let status = registry
            .create_account(register(bad.to_string()))
            .await
            .expect_err("malformed token must be refused");
        assert_eq!(status.code(), Code::InvalidArgument, "{bad:?}");
    }

    // Registering the same token twice is a retry, not a second account.
    let token = AccountKey::generate().auth_token();
    let first = registry
        .create_account(register(token.clone()))
        .await
        .unwrap()
        .into_inner()
        .account
        .unwrap();
    let again = registry
        .create_account(register(token.clone()))
        .await
        .unwrap()
        .into_inner()
        .account
        .unwrap();
    assert_eq!(first, again);

    // And the token, not the key it came from, is the credential.
    let authenticated = registry
        .authenticate(AuthenticateRequest { auth_token: token })
        .await
        .unwrap()
        .into_inner()
        .account
        .unwrap();
    assert_eq!(authenticated.id, first.id);

    server.cleanup().await;
}
