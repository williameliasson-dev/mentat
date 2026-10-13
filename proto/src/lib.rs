//! Generated gRPC types for the mentat sync API, shared by `sync` (server)
//! and `tui` (client).
//!
//! Protos live in `proto/proto/`, one package per service type. Add a new
//! package to `build.rs` in this crate and a matching module below.

/// Code generated from the protos in `proto/` at build time.
pub mod pb {
    pub mod account {
        pub mod v1 {
            tonic::include_proto!("sync.account.v1");
        }
    }
    pub mod vault {
        pub mod v1 {
            tonic::include_proto!("sync.vault.v1");
        }
    }
}

pub use prost_types;
