use std::rc::Rc;

use uuid::Uuid;

use crate::store::{Kind, Node, Vault};
use crate::{CoreError, Folder, Result};

pub struct FolderRepository {
    vault: Rc<Vault>,
}

impl FolderRepository {
    pub(crate) fn new(vault: Rc<Vault>) -> Self {
        Self { vault }
    }

    /// Creates a folder, refusing a name a sibling already holds.
    pub fn create(&self, parent_id: Option<Uuid>, name: &str) -> Result<Folder> {
        if self.list(parent_id)?.iter().any(|f| f.name == name) {
            return Err(CoreError::NameTaken(name.to_string()));
        }
        let id = self.vault.create(Kind::Folder, parent_id, name)?;
        self.vault.save_tree()?;
        self.get(id)
    }

    pub fn get(&self, id: Uuid) -> Result<Folder> {
        Ok(folder(self.vault.get(Kind::Folder, id)?))
    }

    /// Direct children of `parent_id` (`None` = root), by name.
    pub fn list(&self, parent_id: Option<Uuid>) -> Result<Vec<Folder>> {
        let mut folders: Vec<Folder> = self
            .vault
            .children(Kind::Folder, parent_id)?
            .into_iter()
            .map(folder)
            .collect();
        folders.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(folders)
    }

    /// Moves a folder under `parent_id`, or to the root with `None`.
    ///
    /// Cycle-checking is the caller's job — see `FolderService::move_folder`.
    pub fn move_to(&self, id: Uuid, parent_id: Option<Uuid>) -> Result<Folder> {
        self.vault.move_to(Kind::Folder, id, parent_id)?;
        self.vault.save_tree()?;
        self.get(id)
    }

    /// Whether `id` is `ancestor` itself or sits anywhere beneath it.
    pub fn is_descendant_of(&self, id: Uuid, ancestor: Uuid) -> Result<bool> {
        self.vault.is_descendant_of(id, ancestor)
    }

    pub fn rename(&self, id: Uuid, name: &str) -> Result<Folder> {
        self.vault.set_name(Kind::Folder, id, name)?;
        self.vault.save_tree()?;
        self.get(id)
    }

    /// Deletes the folder, and every folder and note inside it.
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.vault.delete(Kind::Folder, id)?;
        self.vault.save_tree()
    }
}

fn folder(node: Node) -> Folder {
    Folder {
        id: node.id,
        parent_id: node.parent,
        name: node.name,
        created_at: node.created_ms / 1000,
    }
}
