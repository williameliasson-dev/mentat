use tui::{CoreError, Database, NoteRepository};

fn repo() -> NoteRepository {
    Database::in_memory().unwrap().repositories().notes
}

#[test]
fn create_and_get() {
    let repo = repo();
    let note = repo.create(None, "Hello", "World").unwrap();
    assert!(note.id > 0);

    let fetched = repo.get(note.id).unwrap();
    assert_eq!(fetched, note);
}

#[test]
fn get_missing_returns_not_found() {
    let repo = repo();
    assert!(matches!(repo.get(42), Err(CoreError::NotFound(42))));
}

#[test]
fn list_orders_by_updated_desc() {
    let repo = repo();
    let a = repo.create(None, "A", "").unwrap();
    let b = repo.create(None, "B", "").unwrap();
    // Touch A so it becomes most recently updated.
    let a = repo.update(a.id, "A2", "").unwrap();

    let ids: Vec<i64> = repo.list(None).unwrap().iter().map(|n| n.id).collect();
    assert_eq!(ids, vec![a.id, b.id]);
}

#[test]
fn update_changes_fields_and_timestamp() {
    let repo = repo();
    let note = repo.create(None, "Old", "Old body").unwrap();
    let updated = repo.update(note.id, "New", "New body").unwrap();

    assert_eq!(updated.title, "New");
    assert_eq!(updated.body, "New body");
    assert_eq!(updated.created_at, note.created_at);
    assert!(updated.updated_at >= note.updated_at);
}

#[test]
fn update_missing_returns_not_found() {
    let repo = repo();
    assert!(matches!(
        repo.update(42, "x", "y"),
        Err(CoreError::NotFound(42))
    ));
}

#[test]
fn delete_removes_note() {
    let repo = repo();
    let note = repo.create(None, "Doomed", "").unwrap();
    repo.delete(note.id).unwrap();
    assert!(matches!(repo.get(note.id), Err(CoreError::NotFound(_))));
}

#[test]
fn delete_missing_returns_not_found() {
    let repo = repo();
    assert!(matches!(repo.delete(42), Err(CoreError::NotFound(42))));
}
