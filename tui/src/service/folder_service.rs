use crate::{Folder, FolderRepository, Result};

pub struct FolderService {
    repository: FolderRepository,
}

impl FolderService {
    pub fn new(repository: FolderRepository) -> Self {
        Self { repository }
    }

    pub fn create_folder(&self, parent_id: Option<i64>, name: &str) -> Result<Folder> {
        self.repository.create(parent_id, name)
    }

    pub fn get_folder(&self, id: i64) -> Result<Folder> {
        self.repository.get(id)
    }

    pub fn list_folders(&self, parent_id: Option<i64>) -> Result<Vec<Folder>> {
        self.repository.list(parent_id)
    }

    pub fn rename_folder(&self, id: i64, name: &str) -> Result<Folder> {
        self.repository.rename(id, name)
    }

    /// Deletes the folder along with everything inside it.
    pub fn delete_folder(&self, id: i64) -> Result<()> {
        self.repository.delete(id)
    }
}
