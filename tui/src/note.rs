use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: Uuid,
    /// Containing folder; `None` at the root.
    pub folder_id: Option<Uuid>,
    pub title: String,
    pub body: String,
    /// Unix timestamp (seconds)
    pub created_at: i64,
    /// Unix timestamp (seconds)
    pub updated_at: i64,
}
