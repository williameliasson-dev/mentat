use mentat_core::{Database, NoteService};

fn service() -> NoteService {
    NoteService::new(Database::in_memory().unwrap().repositories().notes)
}

#[test]
fn full_crud_roundtrip() {
    let svc = service();

    let note = svc.create_note("Shopping", "Milk, eggs").unwrap();
    assert_eq!(svc.list_notes().unwrap().len(), 1);

    let note = svc
        .update_note(note.id, "Shopping", "Milk, eggs, bread")
        .unwrap();
    assert_eq!(note.body, "Milk, eggs, bread");

    assert_eq!(svc.get_note(note.id).unwrap().title, "Shopping");

    svc.delete_note(note.id).unwrap();
    assert!(svc.list_notes().unwrap().is_empty());
}
