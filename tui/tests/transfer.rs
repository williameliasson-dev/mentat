use tui::{CoreError, Database, Services, service::transfer};

fn services() -> Services {
    Services::new(Database::in_memory().unwrap().repositories())
}

// -- Moving ------------------------------------------------------------------

#[test]
fn move_folder_reparents_it() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    let archive = s.folders.create_folder(None, "archive").unwrap();

    let moved = s.folders.move_folder(archive.id, Some(work.id)).unwrap();

    assert_eq!(moved.parent_id, Some(work.id));
    assert!(s.folders.list_folders(None).unwrap() == vec![work]);
}

#[test]
fn move_folder_to_root_clears_the_parent() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    let nested = s.folders.create_folder(Some(work.id), "nested").unwrap();

    assert_eq!(
        s.folders.move_folder(nested.id, None).unwrap().parent_id,
        None
    );
}

#[test]
fn folder_cannot_move_into_itself() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();

    assert!(matches!(
        s.folders.move_folder(work.id, Some(work.id)),
        Err(CoreError::InvalidMove)
    ));
}

#[test]
fn folder_cannot_move_into_its_own_descendant() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    let child = s.folders.create_folder(Some(work.id), "child").unwrap();
    let grandchild = s.folders.create_folder(Some(child.id), "deep").unwrap();

    assert!(matches!(
        s.folders.move_folder(work.id, Some(grandchild.id)),
        Err(CoreError::InvalidMove)
    ));
    // And the tree is untouched.
    assert_eq!(s.folders.get_folder(work.id).unwrap().parent_id, None);
}

#[test]
fn move_onto_a_taken_name_is_rejected() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    s.folders.create_folder(Some(work.id), "notes").unwrap();
    let loose = s.folders.create_folder(None, "notes").unwrap();

    assert!(matches!(
        s.folders.move_folder(loose.id, Some(work.id)),
        Err(CoreError::NameTaken(name)) if name == "notes"
    ));
}

#[test]
fn moving_a_folder_carries_its_contents() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    let inbox = s.folders.create_folder(None, "inbox").unwrap();
    s.notes.create_note(Some(inbox.id), "todo", "body").unwrap();

    s.folders.move_folder(inbox.id, Some(work.id)).unwrap();

    let titles: Vec<String> = s
        .notes
        .list_notes(Some(inbox.id))
        .unwrap()
        .into_iter()
        .map(|n| n.title)
        .collect();
    assert_eq!(titles, vec!["todo"]);
}

// -- Copying -----------------------------------------------------------------

#[test]
fn copy_note_into_another_folder_keeps_the_title() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    let note = s.notes.create_note(None, "todo", "milk").unwrap();

    let copy = transfer::copy_note(&s, &note, Some(work.id)).unwrap();

    assert_ne!(copy.id, note.id);
    assert_eq!(copy.title, "todo");
    assert_eq!(copy.body, "milk");
    assert_eq!(copy.folder_id, Some(work.id));
    // The original stays put.
    assert_eq!(s.notes.get_note(note.id).unwrap().folder_id, None);
}

#[test]
fn copy_note_in_place_gets_a_copy_suffix() {
    let s = services();
    let note = s.notes.create_note(None, "todo", "milk").unwrap();

    assert_eq!(
        transfer::copy_note(&s, &note, None).unwrap().title,
        "todo (copy)"
    );
    assert_eq!(
        transfer::copy_note(&s, &note, None).unwrap().title,
        "todo (copy 2)"
    );
}

#[test]
fn copy_folder_copies_the_whole_subtree() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    let deep = s.folders.create_folder(Some(work.id), "deep").unwrap();
    s.notes.create_note(Some(work.id), "top", "a").unwrap();
    s.notes.create_note(Some(deep.id), "buried", "b").unwrap();
    let dest = s.folders.create_folder(None, "dest").unwrap();

    let copy = transfer::copy_folder(&s, &work, Some(dest.id)).unwrap();

    assert_eq!(copy.name, "work");
    assert_eq!(copy.parent_id, Some(dest.id));

    let top: Vec<String> = s
        .notes
        .list_notes(Some(copy.id))
        .unwrap()
        .into_iter()
        .map(|n| n.title)
        .collect();
    assert_eq!(top, vec!["top"]);

    let subfolders = s.folders.list_folders(Some(copy.id)).unwrap();
    assert_eq!(subfolders.len(), 1);
    assert_eq!(subfolders[0].name, "deep");

    let buried = s.notes.list_notes(Some(subfolders[0].id)).unwrap();
    assert_eq!(buried.len(), 1);
    assert_eq!(buried[0].title, "buried");
    assert_eq!(buried[0].body, "b");
}

#[test]
fn copy_folder_in_place_gets_a_copy_suffix() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    s.notes.create_note(Some(work.id), "todo", "milk").unwrap();

    let copy = transfer::copy_folder(&s, &work, None).unwrap();

    assert_eq!(copy.name, "work (copy)");
    assert_eq!(s.notes.list_notes(Some(copy.id)).unwrap().len(), 1);
    // The original keeps its own note rather than handing it over.
    assert_eq!(s.notes.list_notes(Some(work.id)).unwrap().len(), 1);
}

#[test]
fn folder_cannot_be_copied_into_its_own_subtree() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    let child = s.folders.create_folder(Some(work.id), "child").unwrap();

    assert!(matches!(
        transfer::copy_folder(&s, &work, Some(child.id)),
        Err(CoreError::InvalidMove)
    ));
    assert!(matches!(
        transfer::copy_folder(&s, &work, Some(work.id)),
        Err(CoreError::InvalidMove)
    ));
    // Nothing was created before the guard tripped.
    assert!(s.folders.list_folders(Some(child.id)).unwrap().is_empty());
}
