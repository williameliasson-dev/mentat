use crate::{CoreError, Folder, FolderRepository, Result};

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

    /// Moves a folder under `parent_id` (`None` = root).
    ///
    /// Refuses a move into the folder's own subtree, which would strand it,
    /// and a move onto a name a sibling already holds — the schema's
    /// `UNIQUE (parent_id, name)` would reject that as an opaque SQL error.
    pub fn move_folder(&self, id: i64, parent_id: Option<i64>) -> Result<Folder> {
        if let Some(parent) = parent_id
            && self.repository.is_descendant_of(parent, id)?
        {
            return Err(CoreError::InvalidMove);
        }
        let name = self.repository.get(id)?.name;
        if self
            .repository
            .list(parent_id)?
            .iter()
            .any(|f| f.id != id && f.name == name)
        {
            return Err(CoreError::NameTaken(name));
        }
        self.repository.move_to(id, parent_id)
    }

    /// Whether `id` is `ancestor` or lives inside it.
    pub fn is_descendant_of(&self, id: i64, ancestor: i64) -> Result<bool> {
        self.repository.is_descendant_of(id, ancestor)
    }

    /// Renames a folder, refusing a name a sibling already holds — the
    /// schema's `UNIQUE (parent_id, name)` would otherwise surface as an
    /// opaque SQL error.
    pub fn rename_folder(&self, id: i64, name: &str) -> Result<Folder> {
        let parent_id = self.repository.get(id)?.parent_id;
        if self
            .repository
            .list(parent_id)?
            .iter()
            .any(|f| f.id != id && f.name == name)
        {
            return Err(CoreError::NameTaken(name.to_string()));
        }
        self.repository.rename(id, name)
    }

    /// Deletes the folder along with everything inside it.
    pub fn delete_folder(&self, id: i64) -> Result<()> {
        self.repository.delete(id)
    }
}
