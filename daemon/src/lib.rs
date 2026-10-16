//! The local side of mentat: the vault of notes, stored as Loro documents and
//! synced through the sync server. No UI; the tui builds on this.

pub mod error;
pub mod folder;
pub mod note;
pub mod remote;
pub mod repository;
pub mod service;
pub mod store;

pub use error::{CoreError, Result};
pub use folder::Folder;
pub use note::Note;
pub use remote::{SyncReport, Syncer};
pub use repository::{FolderRepository, NoteRepository, Repositories};
pub use service::{FolderService, NoteService};
pub use store::Store;

/// Every long-lived service, built once at startup.
///
/// Callers are lent this per call rather than holding it, so none keeps a
/// service it doesn't use and none can hold a stale handle. Adding a service
/// means adding a field here and a line in `new`.
pub struct Services {
    pub notes: NoteService,
    pub folders: FolderService,
}

impl Services {
    pub fn new(repositories: Repositories) -> Self {
        Self {
            notes: NoteService::new(repositories.notes),
            folders: FolderService::new(repositories.folders),
        }
    }
}
