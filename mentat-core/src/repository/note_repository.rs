use rusqlite::{Connection, params};

use crate::{CoreError, Note, Result};

pub struct NoteRepository {
    conn: Connection,
}

impl NoteRepository {
    /// Opens (or creates) a database at the given path and runs migrations.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::new(Connection::open(path)?)
    }

    /// Creates an in-memory repository. Useful for tests.
    pub fn in_memory() -> Result<Self> {
        Self::new(Connection::open_in_memory()?)
    }

    fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch(include_str!("seed.sql"))?;
        Ok(Self { conn })
    }

    pub fn create(&self, title: &str, body: &str) -> Result<Note> {
        let now = now_unix();
        self.conn.execute(
            "INSERT INTO notes (title, body, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![title, body, now],
        )?;
        Ok(Note {
            id: self.conn.last_insert_rowid(),
            title: title.to_string(),
            body: body.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get(&self, id: i64) -> Result<Note> {
        self.conn
            .query_row(
                "SELECT id, title, body, created_at, updated_at FROM notes WHERE id = ?1",
                params![id],
                map_note,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound(id),
                other => CoreError::Database(other),
            })
    }

    pub fn list(&self) -> Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, body, created_at, updated_at FROM notes ORDER BY updated_at DESC",
        )?;
        let notes = stmt
            .query_map([], map_note)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(notes)
    }

    pub fn update(&self, id: i64, title: &str, body: &str) -> Result<Note> {
        let changed = self.conn.execute(
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
            .conn
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
        title: row.get(1)?,
        body: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}
