PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS folders (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_id  INTEGER REFERENCES folders(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (parent_id, name)
);

-- `UNIQUE (parent_id, name)` above does not constrain root folders: every NULL
-- is distinct in SQL, so it never collides. This covers that case.
CREATE UNIQUE INDEX IF NOT EXISTS folders_unique_root_name
    ON folders (name) WHERE parent_id IS NULL;

CREATE TABLE IF NOT EXISTS notes (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    title      TEXT NOT NULL,
    body       TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    folder_id  INTEGER REFERENCES folders(id) ON DELETE CASCADE
);
