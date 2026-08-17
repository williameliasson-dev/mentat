use crate::{Note, NoteRepository, Result};

pub struct NoteService {
    repository: NoteRepository,
}

impl NoteService {
    pub fn new(repository: NoteRepository) -> Self {
        Self { repository }
    }

    pub fn create_note(&self, folder_id: Option<i64>, title: &str, body: &str) -> Result<Note> {
        self.repository.create(folder_id, title, body)
    }

    pub fn get_note(&self, id: i64) -> Result<Note> {
        self.repository.get(id)
    }

    pub fn list_notes(&self, folder_id: Option<i64>) -> Result<Vec<Note>> {
        self.repository.list(folder_id)
    }

    pub fn move_note(&self, id: i64, folder_id: Option<i64>) -> Result<Note> {
        self.repository.move_to(id, folder_id)
    }

    pub fn update_note(&self, id: i64, title: &str, body: &str) -> Result<Note> {
        self.repository.update(id, title, body)
    }

    pub fn delete_note(&self, id: i64) -> Result<()> {
        self.repository.delete(id)
    }
}
