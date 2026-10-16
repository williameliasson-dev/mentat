use uuid::Uuid;

/// A folder. `parent_id` is `None` at the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub created_at: i64,
}
