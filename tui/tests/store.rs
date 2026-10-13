//! The vault on disk: everything written must still be there after reopening,
//! the way the app sees it between runs.

use std::path::PathBuf;

use tui::{Services, Store};

struct TempVault(PathBuf);

impl TempVault {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("mentat-vault-{}", uuid::Uuid::new_v4())))
    }

    fn open(&self) -> Services {
        Services::new(Store::open(&self.0).unwrap().repositories())
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn everything_survives_reopening() {
    let vault = TempVault::new();
    let (work, note, archive) = {
        let s = vault.open();
        let work = s.folders.create_folder(None, "work").unwrap();
        let archive = s.folders.create_folder(None, "archive").unwrap();
        let note = s.notes.create_note(Some(work.id), "Plan", "draft").unwrap();
        let note = s.notes.update_note(note.id, "Plan v2", "final").unwrap();
        s.folders.move_folder(archive.id, Some(work.id)).unwrap();
        s.folders.rename_folder(archive.id, "old").unwrap();
        (work, note, archive)
    };

    let s = vault.open();
    assert_eq!(s.notes.get_note(note.id).unwrap(), note);
    assert_eq!(s.notes.list_notes(Some(work.id)).unwrap(), vec![note]);
    let archive = s.folders.get_folder(archive.id).unwrap();
    assert_eq!(
        (archive.name.as_str(), archive.parent_id),
        ("old", Some(work.id))
    );
    assert_eq!(s.folders.list_folders(None).unwrap(), vec![work]);
}

#[test]
fn deleting_a_folder_deletes_its_notes_on_disk() {
    let vault = TempVault::new();
    let s = vault.open();
    let work = s.folders.create_folder(None, "work").unwrap();
    let sub = s.folders.create_folder(Some(work.id), "sub").unwrap();
    let deep = s.notes.create_note(Some(sub.id), "Deep", "body").unwrap();
    let kept = s.notes.create_note(None, "Kept", "body").unwrap();
    let notes_dir = vault.0.join("notes");
    assert!(notes_dir.join(deep.id.to_string()).exists());

    s.folders.delete_folder(work.id).unwrap();
    assert!(!notes_dir.join(deep.id.to_string()).exists());

    let s = vault.open();
    assert!(s.folders.get_folder(sub.id).is_err());
    assert!(s.notes.get_note(deep.id).is_err());
    assert_eq!(s.notes.get_note(kept.id).unwrap().body, "body");
}
