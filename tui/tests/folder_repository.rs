use tui::{CoreError, Database, Repositories};

fn repos() -> Repositories {
    Database::in_memory().unwrap().repositories()
}

#[test]
fn create_and_get() {
    let r = repos();
    let folder = r.folders.create(None, "work").unwrap();
    assert!(folder.id > 0);
    assert_eq!(folder.parent_id, None);
    assert_eq!(r.folders.get(folder.id).unwrap(), folder);
}

#[test]
fn list_root_returns_only_top_level() {
    let r = repos();
    let work = r.folders.create(None, "work").unwrap();
    r.folders.create(Some(work.id), "nested").unwrap();
    r.folders.create(None, "personal").unwrap();

    let names: Vec<String> = r
        .folders
        .list(None)
        .unwrap()
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(names, vec!["personal", "work"]);
}

#[test]
fn list_children_of_folder() {
    let r = repos();
    let work = r.folders.create(None, "work").unwrap();
    r.folders.create(Some(work.id), "nested").unwrap();

    let children = r.folders.list(Some(work.id)).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "nested");
    assert_eq!(children[0].parent_id, Some(work.id));
}

#[test]
fn notes_are_scoped_to_their_folder() {
    let r = repos();
    let work = r.folders.create(None, "work").unwrap();
    r.notes.create(Some(work.id), "In work", "").unwrap();
    r.notes.create(None, "At root", "").unwrap();

    let in_work: Vec<String> = r
        .notes
        .list(Some(work.id))
        .unwrap()
        .into_iter()
        .map(|n| n.title)
        .collect();
    let at_root: Vec<String> = r
        .notes
        .list(None)
        .unwrap()
        .into_iter()
        .map(|n| n.title)
        .collect();

    assert_eq!(in_work, vec!["In work"]);
    assert_eq!(at_root, vec!["At root"]);
}

#[test]
fn deleting_folder_cascades_to_notes_and_subfolders() {
    let r = repos();
    let work = r.folders.create(None, "work").unwrap();
    let sub = r.folders.create(Some(work.id), "sub").unwrap();
    let note = r.notes.create(Some(work.id), "Doomed", "").unwrap();
    let deep = r.notes.create(Some(sub.id), "Also doomed", "").unwrap();

    r.folders.delete(work.id).unwrap();

    // Requires `PRAGMA foreign_keys = ON`; without it these all survive.
    assert!(matches!(r.folders.get(sub.id), Err(CoreError::NotFound(_))));
    assert!(matches!(r.notes.get(note.id), Err(CoreError::NotFound(_))));
    assert!(matches!(r.notes.get(deep.id), Err(CoreError::NotFound(_))));
}

#[test]
fn duplicate_name_in_same_parent_is_rejected() {
    let r = repos();
    r.folders.create(None, "work").unwrap();
    assert!(r.folders.create(None, "work").is_err());
}

#[test]
fn same_name_in_different_parents_is_allowed() {
    let r = repos();
    let a = r.folders.create(None, "a").unwrap();
    let b = r.folders.create(None, "b").unwrap();
    r.folders.create(Some(a.id), "notes").unwrap();
    r.folders.create(Some(b.id), "notes").unwrap();
}

#[test]
fn move_note_between_folders() {
    let r = repos();
    let work = r.folders.create(None, "work").unwrap();
    let note = r.notes.create(None, "Wandering", "").unwrap();

    let moved = r.notes.move_to(note.id, Some(work.id)).unwrap();
    assert_eq!(moved.folder_id, Some(work.id));
    assert_eq!(r.notes.list(None).unwrap().len(), 0);
    assert_eq!(r.notes.list(Some(work.id)).unwrap().len(), 1);

    let back = r.notes.move_to(note.id, None).unwrap();
    assert_eq!(back.folder_id, None);
}

#[test]
fn rename_folder() {
    let r = repos();
    let folder = r.folders.create(None, "typo").unwrap();
    let renamed = r.folders.rename(folder.id, "fixed").unwrap();
    assert_eq!(renamed.name, "fixed");
    assert_eq!(renamed.id, folder.id);
}
