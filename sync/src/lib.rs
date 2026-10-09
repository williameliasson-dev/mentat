pub mod database;
pub mod error;
pub mod grpc;
pub mod model;
pub mod repository;
pub mod service;

/// Code generated from the protos in `proto/` at build time.
///
/// One module per proto package. Add a package (e.g. `sync.vault.v1`) as
/// another nested module here.
pub mod pb {
    pub mod account {
        pub mod v1 {
            tonic::include_proto!("sync.account.v1");
        }
    }
}

pub use database::Database;
pub use error::{Result, SyncError};
pub use model::Account;
pub use repository::{AccountRepository, Repositories};
pub use service::{AccountService, Services};
