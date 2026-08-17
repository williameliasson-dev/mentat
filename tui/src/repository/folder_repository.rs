use std::rc::Rc;

use rusqlite::{Connection, params};

use crate::{CoreError, Folder, Result};

pub struct FolderRepository {
    connection: Rc<Connection>,
}

impl FolderRepository {
    pub(crate) fn new(connection: Rc<Connection>) -> Self {
        Self { connection }
    }

    pub fn create(&self, parent_id: Option<i64>, name: &str) -> Result<Folder> {
        let now = now_unix();
        self.connection.execute(
            "INSERT INTO folders (parent_id, name, created_at) VALUES (?1, ?2, ?3)",
            params![parent_id, name, now],
        )?;
        Ok(Folder {
            id: self.connection.last_insert_rowid(),
            parent_id,
            name: name.to_string(),
            created_at: now,
        })
    }

    pub fn get(&self, id: i64) -> Result<Folder> {
        self.connection
            .query_row(
                "SELECT id, parent_id, name, created_at FROM folders WHERE id = ?1",
                params![id],
                map_folder,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound(id),
                other => CoreError::Database(other),
            })
    }

    /// Direct children of `parent_id`; pass `None` for the root.
    ///
    /// Uses `IS` rather than `=` so a `NULL` parent matches — `= NULL` is
    /// always false in SQL and would silently return nothing at the root.
    pub fn list(&self, parent_id: Option<i64>) -> Result<Vec<Folder>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, parent_id, name, created_at FROM folders
             WHERE parent_id IS ?1 ORDER BY name",
        )?;
        let folders = stmt
            .query_map(params![parent_id], map_folder)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(folders)
    }

    pub fn rename(&self, id: i64, name: &str) -> Result<Folder> {
        let changed = self.connection.execute(
            "UPDATE folders SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        if changed == 0 {
            return Err(CoreError::NotFound(id));
        }
        self.get(id)
    }

    /// Deletes the folder. Subfolders and contained notes go with it, via
    /// `ON DELETE CASCADE` — which requires `PRAGMA foreign_keys = ON`.
    pub fn delete(&self, id: i64) -> Result<()> {
        let changed = self
            .connection
            .execute("DELETE FROM folders WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(CoreError::NotFound(id));
        }
        Ok(())
    }
}

fn map_folder(row: &rusqlite::Row) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        name: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}
