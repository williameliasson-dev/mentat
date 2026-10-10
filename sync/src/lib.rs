pub mod database;
pub mod error;
pub mod grpc;
pub mod model;
pub mod repository;
pub mod service;

/// Generated gRPC types, shared with `tui` via the `proto` crate.
pub use proto;

pub use database::Database;
pub use error::{Result, SyncError};
pub use grpc::{AccountRegistryRpc, AccountsRpc, AuthLayer};
pub use model::Account;
pub use repository::{AccountRepository, Repositories};
pub use service::{AccountService, Services};
