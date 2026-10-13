use std::collections::HashMap;

use bytes::Bytes;
use uuid::Uuid;

use crate::error::Result;
use crate::repository::{LogEntry, VaultRepository};

/// Vault contents are ciphertext the server cannot read, so there is no logic
/// to put here beyond the storage layout; this passes straight through to
/// [`VaultRepository`], and exists so rpcs depend on services like the rest.
#[derive(Clone)]
pub struct VaultService {
    repository: VaultRepository,
}

impl VaultService {
    pub fn new(repository: VaultRepository) -> Self {
        Self { repository }
    }

    pub async fn list_vaults(&self, account: Uuid) -> Result<Vec<Uuid>> {
        self.repository.list_vaults(account).await
    }

    pub async fn get_header(&self, account: Uuid, vault: Uuid) -> Result<(Bytes, String)> {
        self.repository.get_header(account, vault).await
    }

    pub async fn put_header(
        &self,
        account: Uuid,
        vault: Uuid,
        header: Bytes,
        if_match: Option<String>,
    ) -> Result<String> {
        self.repository
            .put_header(account, vault, header, if_match)
            .await
    }

    pub async fn push(
        &self,
        account: Uuid,
        vault: Uuid,
        device: Uuid,
        batch: Bytes,
    ) -> Result<u64> {
        self.repository
            .append_log(account, vault, device, batch)
            .await
    }

    pub async fn log_entries(
        &self,
        account: Uuid,
        vault: Uuid,
        after: &HashMap<Uuid, u64>,
    ) -> Result<Vec<LogEntry>> {
        self.repository.log_entries(account, vault, after).await
    }

    pub async fn read_log(&self, account: Uuid, vault: Uuid, entry: LogEntry) -> Result<Bytes> {
        self.repository.read_log(account, vault, entry).await
    }

    pub async fn prune(&self, account: Uuid, vault: Uuid, device: Uuid, up_to: u64) -> Result<()> {
        self.repository
            .prune_log(account, vault, device, up_to)
            .await
    }

    pub async fn put_snapshot(
        &self,
        account: Uuid,
        vault: Uuid,
        doc: Uuid,
        snapshot: Uuid,
        bytes: Bytes,
    ) -> Result<()> {
        self.repository
            .put_snapshot(account, vault, doc, snapshot, bytes)
            .await
    }

    pub async fn get_snapshot(
        &self,
        account: Uuid,
        vault: Uuid,
        doc: Uuid,
        snapshot: Uuid,
    ) -> Result<Bytes> {
        self.repository
            .get_snapshot(account, vault, doc, snapshot)
            .await
    }

    pub async fn delete_snapshot(
        &self,
        account: Uuid,
        vault: Uuid,
        doc: Uuid,
        snapshot: Uuid,
    ) -> Result<()> {
        self.repository
            .delete_snapshot(account, vault, doc, snapshot)
            .await
    }
}
