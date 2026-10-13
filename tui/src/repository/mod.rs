use std::rc::Rc;

use crate::store::Vault;

pub mod folder_repository;
pub mod note_repository;
pub use folder_repository::FolderRepository;
pub use note_repository::NoteRepository;

/// Every repository, all sharing one vault.
///
/// Built by `Store::repositories`. Adding a repository means adding a field
/// here and a line in `new` — nothing above this layer changes.
pub struct Repositories {
    pub notes: NoteRepository,
    pub folders: FolderRepository,
}

impl Repositories {
    pub(crate) fn new(vault: Rc<Vault>) -> Self {
        Self {
            notes: NoteRepository::new(vault.clone()),
            folders: FolderRepository::new(vault),
        }
    }
}
