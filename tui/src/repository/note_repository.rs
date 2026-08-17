use std::rc::Rc;

use rusqlite::{Connection, params};

use crate::{CoreError, Note, Result};

pub struct NoteRepository {
    connection: Rc<Connection>,
}

impl NoteRepository {
    pub(crate) fn new(connection: Rc<Connection>) -> Self {
        Self { connection }
    }

    pub fn create(&self, folder_id: Option<i64>, title: &str, body: &str) -> Result<Note> {
        let now = now_unix();
        self.connection.execute(
            "INSERT INTO notes (title, body, created_at, updated_at, folder_id)
             VALUES (?1, ?2, ?3, ?3, ?4)",
            params![title, body, now, folder_id],
        )?;
        Ok(Note {
            id: self.connection.last_insert_rowid(),
            folder_id,
            title: title.to_string(),
            body: body.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get(&self, id: i64) -> Result<Note> {
        self.connection
            .query_row(
                "SELECT id, folder_id, title, body, created_at, updated_at
                 FROM notes WHERE id = ?1",
                params![id],
                map_note,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound(id),
                other => CoreError::Database(other),
            })
    }

    /// Notes directly inside `folder_id`; pass `None` for the root.
    ///
    /// Uses `IS` rather than `=` so a `NULL` folder matches — `= NULL` is
    /// always false in SQL and would silently return nothing at the root.
    pub fn list(&self, folder_id: Option<i64>) -> Result<Vec<Note>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, folder_id, title, body, created_at, updated_at FROM notes
             WHERE folder_id IS ?1 ORDER BY updated_at DESC",
        )?;
        let notes = stmt
            .query_map(params![folder_id], map_note)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(notes)
    }

    /// Moves a note into `folder_id`, or to the root with `None`.
    pub fn move_to(&self, id: i64, folder_id: Option<i64>) -> Result<Note> {
        let changed = self.connection.execute(
            "UPDATE notes SET folder_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![folder_id, now_unix(), id],
        )?;
        if changed == 0 {
            return Err(CoreError::NotFound(id));
        }
        self.get(id)
    }

    pub fn update(&self, id: i64, title: &str, body: &str) -> Result<Note> {
        let changed = self.connection.execute(
            "UPDATE notes SET title = ?1, body = ?2, updated_at = ?3 WHERE id = ?4",
            params![title, body, now_unix(), id],
        )?;
        if changed == 0 {
            return Err(CoreError::NotFound(id));
        }
        self.get(id)
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        let changed = self
            .connection
            .execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(CoreError::NotFound(id));
        }
        Ok(())
    }
}

fn map_note(row: &rusqlite::Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        title: row.get(2)?,
        body: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}
