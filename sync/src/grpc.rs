use std::time::SystemTime;

use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::model::Account;
use crate::pb::account::v1::accounts_server::Accounts;
use crate::pb::account::v1::{
    Account as PbAccount, AuthenticateRequest, AuthenticateResponse, CreateAccountRequest,
    CreateAccountResponse, GetAccountRequest, GetAccountResponse,
};
use crate::service::Services;

pub struct AccountsRpc {
    services: Services,
}

impl AccountsRpc {
    pub fn new(services: Services) -> Self {
        Self { services }
    }
}

#[tonic::async_trait]
impl Accounts for AccountsRpc {
    async fn create_account(
        &self,
        _request: Request<CreateAccountRequest>,
    ) -> Result<Response<CreateAccountResponse>, Status> {
        let (account, account_key) = self.services.accounts.register().await?;
        Ok(Response::new(CreateAccountResponse {
            account: Some(pb_account(account)),
            account_key,
        }))
    }

    async fn get_account(
        &self,
        request: Request<GetAccountRequest>,
    ) -> Result<Response<GetAccountResponse>, Status> {
        let id = parse_id(&request)?;
        let account = self.services.accounts.get(id).await?;
        Ok(Response::new(GetAccountResponse {
            account: Some(pb_account(account)),
        }))
    }

    async fn authenticate(
        &self,
        request: Request<AuthenticateRequest>,
    ) -> Result<Response<AuthenticateResponse>, Status> {
        let account_key = request.into_inner().account_key;
        let account = self.services.accounts.authenticate(&account_key).await?;
        Ok(Response::new(AuthenticateResponse {
            account: Some(pb_account(account)),
        }))
    }
}

fn parse_id(request: &Request<GetAccountRequest>) -> std::result::Result<Uuid, Status> {
    Uuid::parse_str(&request.get_ref().id).map_err(|_| Status::invalid_argument("invalid account id"))
}

fn pb_account(account: Account) -> PbAccount {
    PbAccount {
        id: account.id.to_string(),
        created_at: Some(prost_types::Timestamp::from(SystemTime::from(
            account.created_at,
        ))),
    }
}
