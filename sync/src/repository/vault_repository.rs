use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use futures::TryStreamExt;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutMode, UpdateVersion};
use uuid::Uuid;

use crate::error::{Result, SyncError};

/// Every vault lives under `vaults/<account id>/<vault id>/`:
///
/// ```text
/// header                          compare-and-swap document
/// log/<device id>/<seq>.bin       append-only batches, seq zero-padded so
///                                 names sort in numeric order
/// snapshots/<doc id>/<id>.bin     compacted document state
/// ```
///
/// Every method takes the account first and builds the path from it, so the
/// caller's account is the only one a call can ever reach.
#[derive(Clone)]
pub struct VaultRepository {
    store: Arc<dyn ObjectStore>,
}

/// A batch in some device's log, by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogEntry {
    pub device: Uuid,
    pub seq: u64,
}

impl VaultRepository {
    pub(crate) fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }

    pub async fn list_vaults(&self, account: Uuid) -> Result<Vec<Uuid>> {
        let prefix = Path::from_iter(["vaults", &account.to_string()]);
        Ok(child_ids(
            &self
                .store
                .list_with_delimiter(Some(&prefix))
                .await?
                .common_prefixes,
        ))
    }

    /// The header and its etag.
    pub async fn get_header(&self, account: Uuid, vault: Uuid) -> Result<(Bytes, String)> {
        let result = self
            .store
            .get(&vault_path(account, vault).join("header"))
            .await?;
        let etag = result.meta.e_tag.clone().unwrap_or_default();
        Ok((result.bytes().await?, etag))
    }

    /// Creates the header when `if_match` is `None`, otherwise replaces the
    /// version `if_match` names. Either way, losing a race is
    /// [`SyncError::Conflict`]. Returns the new etag.
    pub async fn put_header(
        &self,
        account: Uuid,
        vault: Uuid,
        header: Bytes,
        if_match: Option<String>,
    ) -> Result<String> {
        let mode = match if_match {
            None => PutMode::Create,
            Some(e_tag) => PutMode::Update(UpdateVersion {
                e_tag: Some(e_tag),
                version: None,
            }),
        };
        let result = self
            .store
            .put_opts(
                &vault_path(account, vault).join("header"),
                header.into(),
                mode.into(),
            )
            .await?;
        Ok(result.e_tag.unwrap_or_default())
    }

    /// Appends `batch` to the device's log and returns its seq.
    pub async fn append_log(
        &self,
        account: Uuid,
        vault: Uuid,
        device: Uuid,
        batch: Bytes,
    ) -> Result<u64> {
        let dir = log_dir(account, vault, device);
        loop {
            let seq = self.log_seqs(&dir).await?.last().copied().unwrap_or(0) + 1;
            // Create-only, so two pushes racing for the same seq cannot both
            // land there: the loser sees the winner's batch and moves past it.
            match self
                .store
                .put_opts(
                    &dir.clone().join(log_name(seq)),
                    batch.clone().into(),
                    PutMode::Create.into(),
                )
                .await
            {
                Ok(_) => return Ok(seq),
                Err(object_store::Error::AlreadyExists { .. }) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Every batch past `after` (per device, default 0), in seq order within
    /// each device.
    pub async fn log_entries(
        &self,
        account: Uuid,
        vault: Uuid,
        after: &HashMap<Uuid, u64>,
    ) -> Result<Vec<LogEntry>> {
        let logs = vault_path(account, vault).join("log");
        let devices = child_ids(
            &self
                .store
                .list_with_delimiter(Some(&logs))
                .await?
                .common_prefixes,
        );

        let mut entries = Vec::new();
        for device in devices {
            let after = after.get(&device).copied().unwrap_or(0);
            entries.extend(
                self.log_seqs(&logs.clone().join(device.to_string()))
                    .await?
                    .into_iter()
                    .filter(|&seq| seq > after)
                    .map(|seq| LogEntry { device, seq }),
            );
        }
        Ok(entries)
    }

    pub async fn read_log(&self, account: Uuid, vault: Uuid, entry: LogEntry) -> Result<Bytes> {
        let path = log_dir(account, vault, entry.device).join(log_name(entry.seq));
        Ok(self.store.get(&path).await?.bytes().await?)
    }

    /// Deletes the device's batches up to and including `up_to`, keeping its
    /// newest. Seqs are numbered from the newest batch still stored, so
    /// deleting it would let the next push reuse a seq that clients have
    /// already pulled past, and they would never see that batch.
    pub async fn prune_log(
        &self,
        account: Uuid,
        vault: Uuid,
        device: Uuid,
        up_to: u64,
    ) -> Result<()> {
        let dir = log_dir(account, vault, device);
        let seqs = self.log_seqs(&dir).await?;
        let Some((_newest, older)) = seqs.split_last() else {
            return Ok(());
        };
        for &seq in older.iter().filter(|&&seq| seq <= up_to) {
            self.store.delete(&dir.clone().join(log_name(seq))).await?;
        }
        Ok(())
    }

    pub async fn put_snapshot(
        &self,
        account: Uuid,
        vault: Uuid,
        doc: Uuid,
        snapshot: Uuid,
        bytes: Bytes,
    ) -> Result<()> {
        self.store
            .put(&snapshot_path(account, vault, doc, snapshot), bytes.into())
            .await?;
        Ok(())
    }

    pub async fn get_snapshot(
        &self,
        account: Uuid,
        vault: Uuid,
        doc: Uuid,
        snapshot: Uuid,
    ) -> Result<Bytes> {
        let path = snapshot_path(account, vault, doc, snapshot);
        Ok(self.store.get(&path).await?.bytes().await?)
    }

    pub async fn delete_snapshot(
        &self,
        account: Uuid,
        vault: Uuid,
        doc: Uuid,
        snapshot: Uuid,
    ) -> Result<()> {
        // Deleting what is already gone is not an error: a retried compaction
        // must be able to run again.
        match self
            .store
            .delete(&snapshot_path(account, vault, doc, snapshot))
            .await
        {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(SyncError::from(error)),
        }
    }

    /// The seqs stored in one device's log, ascending.
    async fn log_seqs(&self, dir: &Path) -> Result<Vec<u64>> {
        let mut seqs: Vec<u64> = self
            .store
            .list(Some(dir))
            .try_filter_map(|meta| async move {
                Ok(meta
                    .location
                    .filename()
                    .and_then(|name| name.strip_suffix(".bin"))
                    .and_then(|seq| seq.parse::<u64>().ok()))
            })
            .try_collect()
            .await?;
        seqs.sort_unstable();
        Ok(seqs)
    }
}

fn vault_path(account: Uuid, vault: Uuid) -> Path {
    Path::from_iter(["vaults", &account.to_string(), &vault.to_string()])
}

fn log_dir(account: Uuid, vault: Uuid, device: Uuid) -> Path {
    vault_path(account, vault)
        .join("log")
        .join(device.to_string())
}

fn log_name(seq: u64) -> String {
    format!("{seq:020}.bin")
}

fn snapshot_path(account: Uuid, vault: Uuid, doc: Uuid, snapshot: Uuid) -> Path {
    vault_path(account, vault)
        .join("snapshots")
        .join(doc.to_string())
        .join(format!("{snapshot}.bin"))
}

/// The ids naming each of these "directories". Anything that is not a UUID
/// was not written by us, so it is skipped rather than reported.
fn child_ids(prefixes: &[Path]) -> Vec<Uuid> {
    prefixes
        .iter()
        .filter_map(|prefix| prefix.filename()?.parse().ok())
        .collect()
}
