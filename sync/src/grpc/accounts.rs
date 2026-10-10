use std::time::SystemTime;

use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::grpc::auth::authenticated_account;
use crate::model::Account;
use crate::service::AccountService;
use proto::pb::account::v1::account_registry_server::AccountRegistry;
use proto::pb::account::v1::accounts_server::Accounts;
use proto::pb::account::v1::{
    Account as PbAccount, AuthenticateRequest, AuthenticateResponse, CreateAccountRequest,
    CreateAccountResponse, GetAccountRequest, GetAccountResponse,
};

/// The public account rpcs: minting an account, and trading a key for the
/// account it belongs to.
pub struct AccountRegistryRpc {
    accounts: AccountService,
}

impl AccountRegistryRpc {
    pub fn new(accounts: AccountService) -> Self {
        Self { accounts }
    }
}

#[tonic::async_trait]
impl AccountRegistry for AccountRegistryRpc {
    async fn create_account(
        &self,
        _request: Request<CreateAccountRequest>,
    ) -> Result<Response<CreateAccountResponse>, Status> {
        let (account, account_key) = self.accounts.register().await?;
        Ok(Response::new(CreateAccountResponse {
            account: Some(pb_account(account)),
            account_key,
        }))
    }

    async fn authenticate(
        &self,
        request: Request<AuthenticateRequest>,
    ) -> Result<Response<AuthenticateResponse>, Status> {
        let account_key = request.into_inner().account_key;
        let account = self.accounts.authenticate(&account_key).await?;
        Ok(Response::new(AuthenticateResponse {
            account: Some(pb_account(account)),
        }))
    }
}

/// The account rpcs that require a key. Mounted behind
/// [`AuthLayer`](crate::grpc::AuthLayer), which has already resolved the
/// caller by the time these run — so this holds no service of its own.
#[derive(Default)]
pub struct AccountsRpc;

impl AccountsRpc {
    pub fn new() -> Self {
        Self
    }
}

#[tonic::async_trait]
impl Accounts for AccountsRpc {
    async fn get_account(
        &self,
        request: Request<GetAccountRequest>,
    ) -> Result<Response<GetAccountResponse>, Status> {
        let account = authenticated_account(&request)?;
        let requested = Uuid::parse_str(&request.get_ref().id)
            .map_err(|_| Status::invalid_argument("invalid account id"))?;

        // Authentication said who is calling; this is the authorization half,
        // and the only account anyone may read is their own.
        if requested != account.id {
            return Err(Status::permission_denied("not your account"));
        }

        Ok(Response::new(GetAccountResponse {
            account: Some(pb_account(account)),
        }))
    }
}

fn pb_account(account: Account) -> PbAccount {
    PbAccount {
        id: account.id.to_string(),
        created_at: Some(proto::prost_types::Timestamp::from(SystemTime::from(
            account.created_at,
        ))),
    }
}
