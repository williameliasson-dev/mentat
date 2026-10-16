use std::rc::Rc;

use loro::UpdateOptions;
use uuid::Uuid;

use crate::store::{Kind, Node, Vault};
use crate::{Note, Result};

pub struct NoteRepository {
    vault: Rc<Vault>,
}

impl NoteRepository {
    pub(crate) fn new(vault: Rc<Vault>) -> Self {
        Self { vault }
    }

    pub fn create(&self, folder_id: Option<Uuid>, title: &str, body: &str) -> Result<Note> {
        let id = self.vault.create(Kind::Note, folder_id, title)?;
        self.vault.save_tree()?;
        if !body.is_empty() {
            self.vault.body(id)?.insert(0, body)?;
            self.vault.save_body(id)?;
        }
        self.get(id)
    }

    pub fn get(&self, id: Uuid) -> Result<Note> {
        let node = self.vault.get(Kind::Note, id)?;
        self.note(node)
    }

    /// Notes directly inside `folder_id` (`None` = root), most recently
    /// updated first.
    pub fn list(&self, folder_id: Option<Uuid>) -> Result<Vec<Note>> {
        let mut nodes = self.vault.children(Kind::Note, folder_id)?;
        nodes.sort_by_key(|node| std::cmp::Reverse(node.updated_ms));
        nodes.into_iter().map(|node| self.note(node)).collect()
    }

    /// Moves a note into `folder_id`, or to the root with `None`.
    pub fn move_to(&self, id: Uuid, folder_id: Option<Uuid>) -> Result<Note> {
        self.vault.move_to(Kind::Note, id, folder_id)?;
        self.vault.touch(Kind::Note, id)?;
        self.vault.save_tree()?;
        self.get(id)
    }

    pub fn update(&self, id: Uuid, title: &str, body: &str) -> Result<Note> {
        self.vault.set_name(Kind::Note, id, title)?;
        self.vault.touch(Kind::Note, id)?;
        self.vault.save_tree()?;

        // A diff against the current text rather than a wholesale replace, so
        // an edit made elsewhere to another part of the note survives a merge.
        let text = self.vault.body(id)?;
        if text.to_string() != body {
            text.update(body, UpdateOptions::default())
                .expect("no timeout set");
            self.vault.save_body(id)?;
        }
        self.get(id)
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.vault.delete(Kind::Note, id)?;
        self.vault.save_tree()
    }

    fn note(&self, node: Node) -> Result<Note> {
        Ok(Note {
            body: self.vault.body(node.id)?.to_string(),
            id: node.id,
            folder_id: node.parent,
            title: node.name,
            created_at: node.created_ms / 1000,
            updated_at: node.updated_ms / 1000,
        })
    }
}
