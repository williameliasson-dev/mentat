#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: i64,
    /// Containing folder; `None` at the root.
    pub folder_id: Option<i64>,
    pub title: String,
    pub body: String,
    /// Unix timestamp (seconds)
    pub created_at: i64,
    /// Unix timestamp (seconds)
    pub updated_at: i64,
}
