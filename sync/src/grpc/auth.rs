use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tonic::body::Body;
use tonic::{Request, Status};
use tower::{Layer, Service};

use crate::error::SyncError;
use crate::model::Account;
use crate::service::AccountService;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Rejects requests that do not carry a registered auth token, and hands the
/// resolved [`Account`] to the handler through request extensions.
///
/// Mount it per service, so that which rpcs require a token is answered by where
/// they are registered rather than by matching on request paths:
///
/// ```ignore
/// let auth = AuthLayer::new(services.accounts.clone());
/// Server::builder()
///     .add_service(AccountRegistryServer::new(registry))   // public
///     .add_service(auth.layer(AccountsServer::new(rpc)))   // authenticated
/// ```
#[derive(Clone)]
pub struct AuthLayer {
    accounts: AccountService,
}

impl AuthLayer {
    pub fn new(accounts: AccountService) -> Self {
        Self { accounts }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = Auth<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Auth {
            inner,
            accounts: self.accounts.clone(),
        }
    }
}

/// A service wrapped by [`AuthLayer`].
#[derive(Clone)]
pub struct Auth<S> {
    inner: S,
    accounts: AccountService,
}

impl<S, ReqBody> Service<http::Request<ReqBody>> for Auth<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<std::result::Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: http::Request<ReqBody>) -> Self::Future {
        // The readiness just polled belongs to this `self.inner`, so move that
        // value into the future and leave a fresh clone behind for next time.
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let accounts = self.accounts.clone();

        Box::pin(async move {
            let Some(token) = auth_token(request.headers()) else {
                return Ok(Status::unauthenticated("missing auth token").into_http());
            };

            match accounts.authenticate(&token).await {
                Ok(account) => {
                    request.extensions_mut().insert(account);
                    inner.call(request).await
                }
                Err(SyncError::NotFound) => {
                    Ok(Status::unauthenticated("invalid auth token").into_http())
                }
                // A storage outage is not a bad token; report it as itself.
                Err(error) => Ok(Status::from(error).into_http()),
            }
        })
    }
}

/// The account [`AuthLayer`] resolved for this request.
///
/// Only ever `Err` if an rpc is registered without the layer, which is a
/// wiring mistake rather than anything the caller did.
pub fn authenticated_account<T>(request: &Request<T>) -> std::result::Result<Account, Status> {
    request
        .extensions()
        .get::<Account>()
        .cloned()
        .ok_or_else(|| Status::internal("authenticated rpc mounted without AuthLayer"))
}

fn auth_token(headers: &http::HeaderMap) -> Option<String> {
    headers
        .get(http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_owned)
}

// Wrapping a server must not change which gRPC service it answers for, so the
// routing name passes straight through.
impl<S> tonic::server::NamedService for Auth<S>
where
    S: tonic::server::NamedService,
{
    const NAME: &'static str = S::NAME;
}

#[cfg(test)]
mod tests {
    use super::auth_token;

    fn headers(value: &str) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.insert(http::header::AUTHORIZATION, value.parse().unwrap());
        headers
    }

    #[test]
    fn reads_a_bearer_token() {
        assert_eq!(
            auth_token(&headers("Bearer abc123")).as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn rejects_a_missing_header() {
        assert_eq!(auth_token(&http::HeaderMap::new()), None);
    }

    #[test]
    fn rejects_an_unprefixed_token() {
        assert_eq!(auth_token(&headers("abc123")), None);
    }

    #[test]
    fn rejects_another_scheme() {
        assert_eq!(auth_token(&headers("Basic abc123")), None);
    }
}
