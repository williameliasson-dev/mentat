//! Where notes live: Loro documents on disk.
//!
//! A vault is one directory:
//!
//! ```text
//! tree/          one document: the folder tree, with every folder and note
//!                as a node carrying its id, kind, name and timestamps
//! notes/<id>/    one document per note, holding only its body, opened the
//!                first time the note is read
//! ```
//!
//! Loro documents merge with copies edited elsewhere, which is what sync will
//! build on; nothing here syncs yet.

mod doc_file;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use loro::{LoroMap, LoroText, LoroTree, LoroValue, TreeID, TreeParentId, ValueOrContainer};
use uuid::Uuid;

use crate::{CoreError, Repositories, Result};
use doc_file::DocFile;

/// The open vault, shared by every repository.
///
/// `Rc` and `RefCell` rather than locks, as with the old connection: this
/// crate is single-threaded.
pub struct Store {
    vault: Rc<Vault>,
}

impl Store {
    /// Opens the vault at the platform-idiomatic location, creating one on
    /// first run.
    pub fn new() -> Result<Self> {
        Self::open(default_vault_dir()?)
    }

    /// Opens (or creates) the vault in `dir`.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            vault: Rc::new(Vault::open(dir.as_ref().to_path_buf())?),
        })
    }

    /// An empty vault that never touches disk. Useful for tests.
    pub fn in_memory() -> Result<Self> {
        Ok(Self {
            vault: Rc::new(Vault::in_memory()),
        })
    }

    /// Every repository, each holding a handle to this same vault.
    pub fn repositories(&self) -> Repositories {
        Repositories::new(self.vault.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Folder,
    Note,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Folder => "folder",
            Kind::Note => "note",
        }
    }
}

/// A tree node's fields, as stored in its metadata.
pub(crate) struct Node {
    pub id: Uuid,
    pub parent: Option<Uuid>,
    pub name: String,
    pub created_ms: i64,
    pub updated_ms: i64,
}

/// The tree document and the note documents. Repositories go through this
/// rather than through Loro directly, so the layout above stays in one place.
pub(crate) struct Vault {
    /// `None` for an in-memory vault.
    dir: Option<PathBuf>,
    tree: RefCell<DocFile>,
    /// Every live node, by the id the rest of the app knows it by. Loro's own
    /// `TreeID`s never leave this module.
    nodes: RefCell<HashMap<Uuid, TreeID>>,
    /// Note documents opened so far.
    notes: RefCell<HashMap<Uuid, DocFile>>,
    /// The last timestamp handed out; see [`Vault::now_ms`].
    clock: Cell<i64>,
}

impl Vault {
    fn open(dir: PathBuf) -> Result<Self> {
        Ok(Self::new(
            Some(dir.clone()),
            DocFile::open(dir.join("tree"))?,
        ))
    }

    fn in_memory() -> Self {
        Self::new(None, DocFile::in_memory())
    }

    fn new(dir: Option<PathBuf>, tree: DocFile) -> Self {
        let vault = Self {
            dir,
            tree: RefCell::new(tree),
            nodes: RefCell::default(),
            notes: RefCell::default(),
            clock: Cell::new(0),
        };
        let nodes = vault
            .tree()
            .nodes()
            .into_iter()
            .filter(|&node| !vault.tree().is_node_deleted(&node).unwrap_or(true))
            .filter_map(|node| Some((vault.uuid_of(node)?, node)))
            .collect();
        vault.nodes.replace(nodes);
        vault
    }

    fn tree(&self) -> LoroTree {
        self.tree.borrow().doc.get_tree("tree")
    }

    /// Adds a node and returns its id. Unsaved until [`Vault::save_tree`].
    pub fn create(&self, kind: Kind, parent: Option<Uuid>, name: &str) -> Result<Uuid> {
        let parent = self.parent_id(parent)?;
        let tree = self.tree();
        let node = tree.create(parent)?;
        let id = Uuid::new_v4();
        let now = self.now_ms();

        let meta = tree.get_meta(node)?;
        meta.insert("id", id.to_string())?;
        meta.insert("kind", kind.as_str())?;
        meta.insert("name", name)?;
        meta.insert("created_ms", now)?;
        meta.insert("updated_ms", now)?;

        self.nodes.borrow_mut().insert(id, node);
        Ok(id)
    }

    /// The node `id`, if it exists and is of this kind.
    pub fn get(&self, kind: Kind, id: Uuid) -> Result<Node> {
        let node = self.tree_id(kind, id)?;
        let meta = self.tree().get_meta(node)?;
        Ok(Node {
            id,
            parent: self.parent_of(node),
            name: string(&meta, "name"),
            created_ms: int(&meta, "created_ms"),
            updated_ms: int(&meta, "updated_ms"),
        })
    }

    /// The nodes of this kind directly under `parent` (`None` = root).
    pub fn children(&self, kind: Kind, parent: Option<Uuid>) -> Result<Vec<Node>> {
        let parent = self.parent_id(parent)?;
        self.tree()
            .children(parent)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|node| self.uuid_of(node))
            .filter(|&id| self.kind_of(id) == Some(kind))
            .map(|id| self.get(kind, id))
            .collect()
    }

    pub fn set_name(&self, kind: Kind, id: Uuid, name: &str) -> Result<()> {
        let meta = self.tree().get_meta(self.tree_id(kind, id)?)?;
        if string(&meta, "name") != name {
            meta.insert("name", name)?;
        }
        Ok(())
    }

    pub fn touch(&self, kind: Kind, id: Uuid) -> Result<()> {
        let meta = self.tree().get_meta(self.tree_id(kind, id)?)?;
        meta.insert("updated_ms", self.now_ms())?;
        Ok(())
    }

    /// Moves `id` under `parent` (`None` = root). A move into its own
    /// subtree fails here too, but callers should catch it first and say so.
    pub fn move_to(&self, kind: Kind, id: Uuid, parent: Option<Uuid>) -> Result<()> {
        let node = self.tree_id(kind, id)?;
        self.tree().mov(node, self.parent_id(parent)?)?;
        Ok(())
    }

    /// Deletes `id` and everything under it, note bodies included.
    pub fn delete(&self, kind: Kind, id: Uuid) -> Result<()> {
        let node = self.tree_id(kind, id)?;
        let doomed = self.subtree(node);
        self.tree().delete(node)?;

        let mut nodes = self.nodes.borrow_mut();
        for id in doomed {
            nodes.remove(&id);
            self.remove_body(id)?;
        }
        Ok(())
    }

    /// Whether `id` is `ancestor` or anywhere beneath it.
    pub fn is_descendant_of(&self, id: Uuid, ancestor: Uuid) -> Result<bool> {
        let mut current = Some(id);
        while let Some(id) = current {
            if id == ancestor {
                return Ok(true);
            }
            let node = *self
                .nodes
                .borrow()
                .get(&id)
                .ok_or(CoreError::NotFound(id))?;
            current = self.parent_of(node);
        }
        Ok(false)
    }

    pub fn save_tree(&self) -> Result<()> {
        self.tree.borrow_mut().save()
    }

    /// The body of note `id`, opening its document if this is the first read.
    pub fn body(&self, id: Uuid) -> Result<LoroText> {
        self.tree_id(Kind::Note, id)?;
        let mut notes = self.notes.borrow_mut();
        let file = match notes.entry(id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(match &self.dir {
                Some(dir) => DocFile::open(dir.join("notes").join(id.to_string()))?,
                None => DocFile::in_memory(),
            }),
        };
        Ok(file.doc.get_text("body"))
    }

    pub fn save_body(&self, id: Uuid) -> Result<()> {
        match self.notes.borrow_mut().get_mut(&id) {
            Some(file) => file.save(),
            None => Ok(()),
        }
    }

    /// Strictly increasing milliseconds, so two edits in the same millisecond
    /// still sort in the order they happened.
    fn now_ms(&self) -> i64 {
        let wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_millis() as i64;
        let now = wall.max(self.clock.get() + 1);
        self.clock.set(now);
        now
    }

    fn remove_body(&self, id: Uuid) -> Result<()> {
        let file = match self.notes.borrow_mut().remove(&id) {
            Some(file) => file,
            None => match &self.dir {
                Some(dir) => {
                    let path = dir.join("notes").join(id.to_string());
                    if !path.exists() {
                        return Ok(());
                    }
                    DocFile::open(path)?
                }
                None => return Ok(()),
            },
        };
        file.remove()
    }

    fn tree_id(&self, kind: Kind, id: Uuid) -> Result<TreeID> {
        match self.nodes.borrow().get(&id) {
            Some(&node) if self.kind_of(id) == Some(kind) => Ok(node),
            _ => Err(CoreError::NotFound(id)),
        }
    }

    fn parent_id(&self, parent: Option<Uuid>) -> Result<TreeParentId> {
        match parent {
            None => Ok(TreeParentId::Root),
            Some(id) => Ok(TreeParentId::Node(self.tree_id(Kind::Folder, id)?)),
        }
    }

    fn parent_of(&self, node: TreeID) -> Option<Uuid> {
        match self.tree().parent(node)? {
            TreeParentId::Node(parent) => self.uuid_of(parent),
            _ => None,
        }
    }

    fn kind_of(&self, id: Uuid) -> Option<Kind> {
        let node = *self.nodes.borrow().get(&id)?;
        match string(&self.tree().get_meta(node).ok()?, "kind").as_str() {
            "folder" => Some(Kind::Folder),
            "note" => Some(Kind::Note),
            _ => None,
        }
    }

    fn uuid_of(&self, node: TreeID) -> Option<Uuid> {
        string(&self.tree().get_meta(node).ok()?, "id").parse().ok()
    }

    /// `node` and every node beneath it.
    fn subtree(&self, node: TreeID) -> Vec<Uuid> {
        let mut ids = Vec::new();
        let mut stack = vec![node];
        while let Some(node) = stack.pop() {
            ids.extend(self.uuid_of(node));
            stack.extend(self.tree().children(node).unwrap_or_default());
        }
        ids
    }
}

fn string(meta: &LoroMap, key: &str) -> String {
    match meta.get(key) {
        Some(ValueOrContainer::Value(LoroValue::String(s))) => s.to_string(),
        _ => String::new(),
    }
}

fn int(meta: &LoroMap, key: &str) -> i64 {
    match meta.get(key) {
        Some(ValueOrContainer::Value(LoroValue::I64(n))) => n,
        _ => 0,
    }
}

/// `vaults/<id>/` under the platform data directory, the first one found, or a
/// new one on first run:
/// Linux: `~/.local/share/mentat/vaults/`
/// macOS: `~/Library/Application Support/dev.mentat/vaults/`
fn default_vault_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("dev", "", "mentat")
        .expect("could not determine data directory");
    let vaults = dirs.data_dir().join("vaults");
    fs::create_dir_all(&vaults)?;

    let mut existing: Vec<PathBuf> = fs::read_dir(&vaults)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.parse::<Uuid>().is_ok())
        })
        .collect();
    existing.sort();
    Ok(existing
        .into_iter()
        .next()
        .unwrap_or_else(|| vaults.join(Uuid::new_v4().to_string())))
}
