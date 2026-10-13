use std::sync::Arc;

use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use object_store::memory::InMemory;
use object_store::prefix::PrefixStore;

use crate::repository::Repositories;

/// The bucket everything lives in: accounts and every vault's encrypted data.
///
/// There is no database; each repository reads and writes objects here. The
/// bucket must enforce conditional PUTs (`If-None-Match` / `If-Match`), since
/// account creation, vault headers and log numbering rely on them. S3, R2 and
/// versitygw do; Garage ignores them.
pub struct Storage {
    pub store: Arc<dyn ObjectStore>,
}

impl Storage {
    /// Connects to the bucket named by `S3_ENDPOINT`, `S3_BUCKET`,
    /// `S3_REGION`, `S3_ACCESS_KEY_ID` and `S3_SECRET_ACCESS_KEY`.
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let var = |name: &str| std::env::var(name).map_err(|_| format!("{name} is not set"));
        let endpoint = var("S3_ENDPOINT")?;

        let store = AmazonS3Builder::new()
            // Plain http is only ever a local dev endpoint.
            .with_allow_http(endpoint.starts_with("http://"))
            .with_endpoint(endpoint)
            .with_bucket_name(var("S3_BUCKET")?)
            .with_region(var("S3_REGION")?)
            .with_access_key_id(var("S3_ACCESS_KEY_ID")?)
            .with_secret_access_key(var("S3_SECRET_ACCESS_KEY")?)
            .build()?;
        Ok(Self {
            store: Arc::new(store),
        })
    }

    /// An empty in-memory bucket. Useful for tests.
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(InMemory::new()),
        }
    }

    /// The same bucket, seen only below `prefix`, so tests sharing a bucket
    /// cannot collide and can clean up after themselves.
    pub fn with_prefix(self, prefix: &str) -> Self {
        Self {
            store: Arc::new(PrefixStore::new(self.store, prefix)),
        }
    }

    /// Every repository, each holding a handle to this same bucket.
    pub fn repositories(&self) -> Repositories {
        Repositories::new(self.store.clone())
    }
}
