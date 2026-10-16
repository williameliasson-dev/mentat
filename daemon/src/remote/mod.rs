//! Syncing a vault with the sync server.
//!
//! A sync pushes this device's unsynced changes, then pulls every other
//! device's, all as encrypted batches the server cannot read:
//!
//! 1. **Link**, once: register the account's auth token, and pick the vault to
//!    sync with: the account's existing one if it has any, so a second device
//!    joins the first, otherwise this vault under its own id.
//! 2. **Push**: every document saved since the last sync contributes its
//!    changes past what the server has, sealed into batches under this
//!    device's id.
//! 3. **Pull**: every batch other devices pushed since the last pull is
//!    opened and merged in.
//!
//! Nothing else touches the vault while this runs, so once it finishes the
//! server holds everything local and vice versa. A sync that dies halfway
//! just redoes some work next time; Loro ignores updates it already has.

mod batch;
mod state;

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use crypto::{AccountKey, VaultKey};
use proto::pb::account::v1::CreateAccountRequest;
use proto::pb::account::v1::account_registry_client::AccountRegistryClient;
use proto::pb::vault::v1::vaults_client::VaultsClient;
use proto::pb::vault::v1::{ListVaultsRequest, PullRequest, PushRequest};
use tonic::metadata::MetadataValue;
use tonic::service::Interceptor;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;
use uuid::Uuid;

use crate::store::{DocRef, Vault};
use crate::{CoreError, Result, Store};
use state::SyncState;

/// Batches stay well under the server's 4 MB message limit.
const MAX_BATCH: usize = 1024 * 1024;

/// What one sync moved.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    pub pushed: usize,
    pub pulled: usize,
}

pub struct Syncer {
    url: String,
    account: AccountKey,
    runtime: tokio::runtime::Runtime,
}

impl Syncer {
    pub fn new(url: impl Into<String>, account: AccountKey) -> Result<Self> {
        Ok(Self {
            url: url.into(),
            account,
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?,
        })
    }

    /// A syncer for the server at `MENTAT_SYNC_URL`, using the account key in
    /// the data directory (created on first use), or `None` when the variable
    /// is unset and the app should stay offline.
    pub fn from_env() -> Result<Option<Self>> {
        let Ok(url) = std::env::var("MENTAT_SYNC_URL") else {
            return Ok(None);
        };
        Ok(Some(Self::new(url, load_account_key()?)?))
    }

    /// Pushes, then pulls. Blocks until done.
    pub fn sync(&self, store: &Store) -> Result<SyncReport> {
        let vault = store.vault();
        let dir = vault
            .dir()
            .ok_or_else(|| CoreError::Sync("an in-memory vault cannot sync".into()))?;
        let mut state = SyncState::load(dir)?;
        self.runtime.block_on(self.run(vault, &mut state, dir))
    }

    async fn run(&self, vault: &Vault, state: &mut SyncState, dir: &Path) -> Result<SyncReport> {
        let channel = Channel::from_shared(self.url.clone())
            .map_err(|e| CoreError::Sync(format!("bad sync url: {e}")))?
            .connect()
            .await?;
        let mut vaults =
            VaultsClient::with_interceptor(channel.clone(), Bearer::new(&self.account));

        let remote = match state.remote_vault {
            Some(remote) => remote,
            None => {
                let remote = self.link(channel, &mut vaults, dir).await?;
                state.remote_vault = Some(remote);
                state.save()?;
                remote
            }
        };
        let key = self.account.vault_key(remote);

        let pushed = push(vault, state, &mut vaults, remote, &key).await?;
        let pulled = pull(vault, state, &mut vaults, remote, &key).await?;
        Ok(SyncReport { pushed, pulled })
    }

    async fn link(&self, channel: Channel, vaults: &mut Client, dir: &Path) -> Result<Uuid> {
        // Registering is idempotent, so a link that failed after this point
        // can simply run again.
        AccountRegistryClient::new(channel)
            .create_account(CreateAccountRequest {
                auth_token: self.account.auth_token(),
            })
            .await?;

        let existing = vaults
            .list_vaults(ListVaultsRequest {})
            .await?
            .into_inner()
            .vault_ids;
        // The vault directory is named by its id; fall back to a fresh one
        // for a directory named anything else.
        let local = || {
            dir.file_name()
                .and_then(|name| name.to_str()?.parse().ok())
                .unwrap_or_else(Uuid::new_v4)
        };
        Ok(existing
            .iter()
            .filter_map(|id| id.parse().ok())
            .min()
            .unwrap_or_else(local))
    }
}

type Client = VaultsClient<InterceptedService<Channel, Bearer>>;

async fn push(
    vault: &Vault,
    state: &mut SyncState,
    vaults: &mut Client,
    remote: Uuid,
    key: &VaultKey,
) -> Result<usize> {
    let mut batches: Vec<Vec<(DocRef, Vec<u8>)>> = vec![Vec::new()];
    let mut size = 0;
    for doc in vault.unsynced() {
        let since = state.pushed(doc);
        if vault.version(doc)? == since {
            continue;
        }
        let Some(update) = vault.export(doc, &since)? else {
            continue;
        };
        if size + update.len() > MAX_BATCH && size > 0 {
            batches.push(Vec::new());
            size = 0;
        }
        size += update.len();
        batches.last_mut().expect("never empty").push((doc, update));
    }

    let mut pushed = 0;
    for entries in batches.iter().filter(|entries| !entries.is_empty()) {
        let sealed = key.seal(&batch::encode(entries), &context(state.device));
        let seq = vaults
            .push(PushRequest {
                vault_id: remote.to_string(),
                device_id: state.device.to_string(),
                batch: sealed,
            })
            .await?
            .into_inner()
            .seq;
        // Our own batches never need pulling back.
        state.cursors.insert(state.device, seq);
        pushed += 1;
    }

    for doc in vault.unsynced() {
        state.set_pushed(doc, &vault.version(doc)?);
    }
    // State first: if this dies in between, the next push finds nothing new
    // in these documents, rather than forgetting that they changed.
    state.save()?;
    vault.clear_unsynced()?;
    Ok(pushed)
}

async fn pull(
    vault: &Vault,
    state: &mut SyncState,
    vaults: &mut Client,
    remote: Uuid,
    key: &VaultKey,
) -> Result<usize> {
    let mut stream = vaults
        .pull(PullRequest {
            vault_id: remote.to_string(),
            after: state
                .cursors
                .iter()
                .map(|(device, &seq)| (device.to_string(), seq))
                .collect(),
        })
        .await?
        .into_inner();

    let mut pulled = 0;
    while let Some(message) = stream.message().await? {
        let device: Uuid = message
            .device_id
            .parse()
            .map_err(|_| CoreError::Sync("server sent a bad device id".into()))?;
        let plain = key.open(&message.batch, &context(device))?;
        for (doc, update) in batch::decode(&plain)? {
            vault.import(doc, &update)?;
            // Pushed first, so nothing here is unsynced: everything this
            // document now holds, the server has.
            state.set_pushed(doc, &vault.version(doc)?);
        }
        state.cursors.insert(device, message.seq);
        pulled += 1;
    }
    state.save()?;
    Ok(pulled)
}

/// Binds a batch to the device whose log it sits in, so the server cannot
/// pass one device's batch off as another's.
fn context(device: Uuid) -> Vec<u8> {
    [b"mentat/v1/batch/".as_slice(), device.as_bytes()].concat()
}

/// Adds the account's auth token to every request.
#[derive(Clone)]
pub struct Bearer(MetadataValue<tonic::metadata::Ascii>);

impl Bearer {
    fn new(account: &AccountKey) -> Self {
        let token = format!("Bearer {}", account.auth_token());
        Self(token.parse().expect("hex is valid metadata"))
    }
}

impl Interceptor for Bearer {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> std::result::Result<tonic::Request<()>, tonic::Status> {
        request
            .metadata_mut()
            .insert("authorization", self.0.clone());
        Ok(request)
    }
}

/// The account key in the data directory, generated on first use. Readable
/// by the owner only, since it decrypts everything.
fn load_account_key() -> Result<AccountKey> {
    let path = account_key_path();
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text.parse()?),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let key = AccountKey::generate();
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            let mut file = options.open(&path)?;
            writeln!(file, "{key}")?;
            file.sync_all()?;
            Ok(key)
        }
        Err(e) => Err(e.into()),
    }
}

/// `account.key` in the platform data directory, beside `vaults/`.
fn account_key_path() -> PathBuf {
    let dirs = directories::ProjectDirs::from("dev", "", "mentat")
        .expect("could not determine data directory");
    let dir = dirs.data_dir();
    fs::create_dir_all(dir).expect("failed to create data directory");
    dir.join("account.key")
}
