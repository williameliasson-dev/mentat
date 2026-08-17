use std::rc::Rc;

use rusqlite::Connection;

pub mod note_repository;
pub use note_repository::NoteRepository;

/// Every repository, all sharing one connection.
///
/// Built by `Database::repositories`. Adding a repository means adding a field
/// here and a line in `new` — nothing above this layer changes.
pub struct Repositories {
    pub notes: NoteRepository,
}

impl Repositories {
    pub(crate) fn new(connection: Rc<Connection>) -> Self {
        Self {
            notes: NoteRepository::new(connection.clone()),
        }
    }
}
