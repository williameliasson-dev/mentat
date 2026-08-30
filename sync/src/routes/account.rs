use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use garde::Validate;
use serde::{Deserialize, Serialize};

use crate::Services;
use crate::error::Result;
use crate::extract::ValidatedJson;
use crate::model::Account;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAccountRequest {}

#[derive(Debug, Serialize)]
pub struct CreateAccountResponse {
    pub account: Account,
    pub account_key: String,
}

pub async fn create_account(
    State(services): State<Arc<Services>>,
    ValidatedJson(_request): ValidatedJson<CreateAccountRequest>,
) -> Result<Json<CreateAccountResponse>> {
    let (account, account_key) = services.accounts.register().await?;
    Ok(Json(CreateAccountResponse {
        account,
        account_key,
    }))
}
