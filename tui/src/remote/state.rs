use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use loro::VersionVector;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Result;
use crate::store::DocRef;

/// What a vault remembers between syncs, in `sync.json` beside its documents.
///
/// Losing it is safe, only wasteful: the next sync re-pushes and re-pulls
/// everything, and Loro ignores what it already has.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SyncState {
    /// This device's id: names its log on the server.
    pub device: Uuid,
    /// The server vault this one syncs with, once linked.
    pub remote_vault: Option<Uuid>,
    /// Per device, the last batch already pulled.
    pub cursors: HashMap<Uuid, u64>,
    /// Per document (by [`DocRef::key`]), the version the server already has,
    /// as an encoded Loro version vector in hex.
    pushed: HashMap<String, String>,
    #[serde(skip)]
    path: PathBuf,
}

impl SyncState {
    pub fn load(vault_dir: &Path) -> Result<Self> {
        let path = vault_dir.join("sync.json");
        match fs::read(&path) {
            Ok(bytes) => Ok(Self {
                path,
                ..serde_json::from_slice(&bytes)?
            }),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self {
                device: Uuid::new_v4(),
                remote_vault: None,
                cursors: HashMap::new(),
                pushed: HashMap::new(),
                path,
            }),
            Err(e) => Err(e.into()),
        }
    }

    /// Writes the state out, atomically.
    pub fn save(&self) -> Result<()> {
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(&temp, &self.path)?;
        Ok(())
    }

    /// The version of `doc` the server has; empty if it has none of it.
    pub fn pushed(&self, doc: DocRef) -> VersionVector {
        self.pushed
            .get(&doc.key())
            .and_then(|hex| hex::decode(hex).ok())
            .and_then(|bytes| VersionVector::decode(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn set_pushed(&mut self, doc: DocRef, version: &VersionVector) {
        self.pushed.insert(doc.key(), hex::encode(version.encode()));
    }
}
