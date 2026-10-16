use daemon::{CoreError, NoteRepository, Store};
use uuid::Uuid;

fn repo() -> NoteRepository {
    Store::in_memory().unwrap().repositories().notes
}

#[test]
fn create_and_get() {
    let repo = repo();
    let note = repo.create(None, "Hello", "World").unwrap();

    let fetched = repo.get(note.id).unwrap();
    assert_eq!(fetched, note);
}

#[test]
fn get_missing_returns_not_found() {
    let repo = repo();
    let missing = Uuid::new_v4();
    assert!(matches!(repo.get(missing), Err(CoreError::NotFound(id)) if id == missing));
}

#[test]
fn list_orders_by_updated_desc() {
    let repo = repo();
    let a = repo.create(None, "A", "").unwrap();
    let b = repo.create(None, "B", "").unwrap();
    // Touch A so it becomes most recently updated.
    let a = repo.update(a.id, "A2", "").unwrap();

    let ids: Vec<Uuid> = repo.list(None).unwrap().iter().map(|n| n.id).collect();
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
    let missing = Uuid::new_v4();
    assert!(matches!(
        repo.update(missing, "x", "y"),
        Err(CoreError::NotFound(id)) if id == missing
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
    let missing = Uuid::new_v4();
    assert!(matches!(repo.delete(missing), Err(CoreError::NotFound(id)) if id == missing));
}
