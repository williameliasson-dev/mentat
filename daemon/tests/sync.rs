//! Two devices syncing through a real sync server, run in-process over an
//! in-memory bucket. Each device is its own vault directory; both hold the
//! same account key, the way a second device is set up.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use crypto::AccountKey;
use daemon::{Services, Store, SyncReport, Syncer};
use futures::TryStreamExt;
use object_store::{ObjectStore, ObjectStoreExt};
use proto::pb::account::v1::account_registry_server::AccountRegistryServer;
use proto::pb::vault::v1::vaults_server::VaultsServer;
use sync::grpc::{AccountRegistryRpc, AuthLayer, VaultsRpc};
use tokio_stream::wrappers::TcpListenerStream;
use tower::Layer;

struct Server {
    url: String,
    store: Arc<dyn ObjectStore>,
}

impl Server {
    /// Serves from its own thread, since the devices block on their own
    /// runtimes while syncing.
    fn start() -> Self {
        let storage = sync::Storage::in_memory();
        let store = storage.store.clone();
        let services = sync::Services::new(storage.repositories());
        let (addr_tx, addr_rx) = std::sync::mpsc::channel::<SocketAddr>();

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                addr_tx.send(listener.local_addr().unwrap()).unwrap();
                let auth = AuthLayer::new(services.accounts.clone());
                tonic::transport::Server::builder()
                    .add_service(AccountRegistryServer::new(AccountRegistryRpc::new(
                        services.accounts.clone(),
                    )))
                    .add_service(
                        auth.layer(VaultsServer::new(VaultsRpc::new(services.vaults.clone()))),
                    )
                    .serve_with_incoming(TcpListenerStream::new(listener))
                    .await
                    .unwrap();
            });
        });

        Self {
            url: format!("http://{}", addr_rx.recv().unwrap()),
            store,
        }
    }

    /// Every byte the server has stored.
    fn stored_bytes(&self) -> Vec<u8> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let paths: Vec<_> = self
                .store
                .list(None)
                .map_ok(|meta| meta.location)
                .try_collect()
                .await
                .unwrap();
            let mut all = Vec::new();
            for path in paths {
                all.extend(self.store.get(&path).await.unwrap().bytes().await.unwrap());
            }
            all
        })
    }
}

/// One device: a vault directory, removed afterwards.
struct Device {
    dir: PathBuf,
    syncer: Syncer,
    store: Store,
    services: Services,
}

impl Device {
    fn new(server: &Server, account: &AccountKey) -> Self {
        let dir = std::env::temp_dir()
            .join("mentat-sync-test")
            .join(uuid::Uuid::new_v4().to_string());
        let store = Store::open(&dir).unwrap();
        Self {
            syncer: Syncer::new(server.url.clone(), account.clone()).unwrap(),
            services: Services::new(store.repositories()),
            store,
            dir,
        }
    }

    fn sync(&self) -> SyncReport {
        self.syncer.sync(&self.store).unwrap()
    }

    /// The same vault, as the app sees it after a restart.
    fn reopen(&mut self) {
        self.store = Store::open(&self.dir).unwrap();
        self.services = Services::new(self.store.repositories());
    }

    fn titles(&self, folder: Option<uuid::Uuid>) -> Vec<String> {
        let mut titles: Vec<String> = self
            .services
            .notes
            .list_notes(folder)
            .unwrap()
            .into_iter()
            .map(|n| n.title)
            .collect();
        titles.sort();
        titles
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_second_device_gets_everything() {
    let server = Server::start();
    let account = AccountKey::generate();
    let laptop = Device::new(&server, &account);
    let work = laptop.services.folders.create_folder(None, "work").unwrap();
    let note = laptop
        .services
        .notes
        .create_note(Some(work.id), "Plan", "ship it")
        .unwrap();
    laptop.sync();

    let desktop = Device::new(&server, &account);
    let report = desktop.sync();
    assert_eq!(report.pulled, 1);
    assert_eq!(
        desktop.services.folders.list_folders(None).unwrap(),
        vec![work.clone()]
    );
    assert_eq!(
        desktop.services.notes.get_note(note.id).unwrap().body,
        "ship it"
    );

    // And nothing moves twice: both sides are now up to date.
    assert_eq!(laptop.sync(), SyncReport::default());
    assert_eq!(desktop.sync(), SyncReport::default());
}

#[test]
fn concurrent_edits_to_one_note_merge() {
    let server = Server::start();
    let account = AccountKey::generate();
    let laptop = Device::new(&server, &account);
    let desktop = Device::new(&server, &account);
    let note = laptop
        .services
        .notes
        .create_note(None, "Todo", "milk\neggs\n")
        .unwrap();
    laptop.sync();
    desktop.sync();

    // Offline on both, different parts of the same note.
    laptop
        .services
        .notes
        .update_note(note.id, "Todo", "MILK\neggs\n")
        .unwrap();
    desktop
        .services
        .notes
        .update_note(note.id, "Todo", "milk\neggs\nbread\n")
        .unwrap();
    laptop.sync();
    desktop.sync();
    laptop.sync();

    for device in [&laptop, &desktop] {
        assert_eq!(
            device.services.notes.get_note(note.id).unwrap().body,
            "MILK\neggs\nbread\n"
        );
    }
}

#[test]
fn moves_renames_and_deletes_reach_the_other_device() {
    let server = Server::start();
    let account = AccountKey::generate();
    let laptop = Device::new(&server, &account);
    let desktop = Device::new(&server, &account);
    let s = &laptop.services;
    let work = s.folders.create_folder(None, "work").unwrap();
    let old = s.folders.create_folder(None, "old").unwrap();
    let note = s.notes.create_note(None, "Loose", "").unwrap();
    let doomed = s.notes.create_note(Some(old.id), "Doomed", "").unwrap();
    laptop.sync();
    desktop.sync();

    s.notes.move_note(note.id, Some(work.id)).unwrap();
    s.folders.rename_folder(work.id, "job").unwrap();
    s.folders.delete_folder(old.id).unwrap();
    laptop.sync();
    desktop.sync();

    let d = &desktop.services;
    assert_eq!(d.folders.get_folder(work.id).unwrap().name, "job");
    assert_eq!(desktop.titles(Some(work.id)), vec!["Loose"]);
    assert!(desktop.titles(None).is_empty());
    assert!(d.folders.get_folder(old.id).is_err());
    assert!(d.notes.get_note(doomed.id).is_err());
}

#[test]
fn edits_made_before_a_restart_still_sync() {
    let server = Server::start();
    let account = AccountKey::generate();
    let mut laptop = Device::new(&server, &account);
    let mut desktop = Device::new(&server, &account);
    laptop.sync();

    // Edited and closed without syncing, as if the app died before quitting.
    laptop
        .services
        .notes
        .create_note(None, "Unsynced", "still here")
        .unwrap();
    laptop.reopen();
    assert_eq!(laptop.sync().pushed, 1);

    desktop.sync();
    assert_eq!(desktop.titles(None), vec!["Unsynced"]);
    // The desktop's own progress survives its restart too.
    desktop.reopen();
    assert_eq!(desktop.sync(), SyncReport::default());
}

#[test]
fn the_server_only_ever_sees_ciphertext() {
    let server = Server::start();
    let account = AccountKey::generate();
    let laptop = Device::new(&server, &account);
    let folder = laptop
        .services
        .folders
        .create_folder(None, "folder-name-canary")
        .unwrap();
    laptop
        .services
        .notes
        .create_note(Some(folder.id), "title-canary", "body-canary")
        .unwrap();
    laptop.sync();

    let stored = server.stored_bytes();
    assert!(!stored.is_empty());
    for canary in ["folder-name-canary", "title-canary", "body-canary"] {
        assert!(
            !stored.windows(canary.len()).any(|w| w == canary.as_bytes()),
            "{canary} reached the server in plaintext"
        );
    }

    // And another account cannot pull it, even knowing the vault's id.
    let stranger = Device::new(&server, &AccountKey::generate());
    stranger.sync();
    assert!(stranger.titles(None).is_empty());
    assert!(
        stranger
            .services
            .folders
            .list_folders(None)
            .unwrap()
            .is_empty()
    );
}
