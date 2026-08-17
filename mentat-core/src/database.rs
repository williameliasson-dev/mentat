use std::path::{Path, PathBuf};
use std::rc::Rc;

use rusqlite::Connection;

use crate::{Repositories, Result};

/// The one database connection, shared by every repository.
///
/// `Rc` needs no lock because every `Connection` method takes `&self`. The
/// trade-off is that `Rc` is `!Send`, so this crate is single-threaded. Moving
/// to `Arc<Mutex<_>>` later is confined to this file and the repositories —
/// nothing above them touches the `Connection`.
pub struct Database {
    connection: Rc<Connection>,
}

impl Database {
    /// Opens the database at the platform-idiomatic location and migrates it.
    pub fn new() -> Result<Self> {
        Self::open(default_path())
    }

    /// Opens (or creates) a database at `path` and runs migrations once.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::migrate(Connection::open(path)?)
    }

    /// In-memory database, migrated. Useful for tests.
    pub fn in_memory() -> Result<Self> {
        Self::migrate(Connection::open_in_memory()?)
    }

    fn migrate(connection: Connection) -> Result<Self> {
        connection.execute_batch(include_str!("schema.sql"))?;
        Ok(Self {
            connection: Rc::new(connection),
        })
    }

    /// Every repository, each holding a handle to this same connection.
    pub fn repositories(&self) -> Repositories {
        Repositories::new(self.connection.clone())
    }
}

/// Platform-idiomatic database location:
/// Linux: `~/.local/share/mentat/mentat.db`
/// macOS: `~/Library/Application Support/dev.mentat/mentat.db`
fn default_path() -> PathBuf {
    let dirs = directories::ProjectDirs::from("dev", "", "mentat")
        .expect("could not determine data directory");
    let dir = dirs.data_dir();
    std::fs::create_dir_all(dir).expect("failed to create data directory");
    dir.join("mentat.db")
}
