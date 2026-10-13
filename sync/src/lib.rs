pub mod error;
pub mod grpc;
pub mod model;
pub mod repository;
pub mod service;
pub mod storage;

/// Generated gRPC types, shared with `tui` via the `proto` crate.
pub use proto;

pub use error::{Result, SyncError};
pub use grpc::{AccountRegistryRpc, AccountsRpc, AuthLayer, VaultsRpc};
pub use model::Account;
pub use repository::{AccountRepository, Repositories, VaultRepository};
pub use service::{AccountService, Services, VaultService};
pub use storage::Storage;
