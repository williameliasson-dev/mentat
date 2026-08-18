//! Copying entries around the tree.
//!
//! These are free functions over `Services` rather than methods on either
//! service, because copying a folder has to create notes as well — neither
//! service owns the whole job.

use crate::{CoreError, Folder, Note, Result, Services};

/// Copies `note` into `into` (`None` = root), under a title no sibling holds.
pub fn copy_note(services: &Services, note: &Note, into: Option<i64>) -> Result<Note> {
    let titles: Vec<String> = services
        .notes
        .list_notes(into)?
        .into_iter()
        .map(|n| n.title)
        .collect();
    services
        .notes
        .create_note(into, &free_name(&note.title, &titles), &note.body)
}

/// Recursively copies `folder` and everything under it into `into`.
///
/// Only the top-level name is deduplicated: everything below lands in a
/// freshly created folder, where nothing can collide.
pub fn copy_folder(services: &Services, folder: &Folder, into: Option<i64>) -> Result<Folder> {
    // A folder copied into its own subtree would keep finding the copy it just
    // made as it recursed — the copy grows forever.
    if let Some(dest) = into
        && services.folders.is_descendant_of(dest, folder.id)?
    {
        return Err(CoreError::InvalidMove);
    }

    let names: Vec<String> = services
        .folders
        .list_folders(into)?
        .into_iter()
        .map(|f| f.name)
        .collect();
    let copy = services
        .folders
        .create_folder(into, &free_name(&folder.name, &names))?;

    for note in services.notes.list_notes(Some(folder.id))? {
        services
            .notes
            .create_note(Some(copy.id), &note.title, &note.body)?;
    }
    for child in services.folders.list_folders(Some(folder.id))? {
        copy_folder(services, &child, Some(copy.id))?;
    }
    Ok(copy)
}

/// `name`, else `name (copy)`, `name (copy 2)`, … — the first one `taken`
/// doesn't already contain.
fn free_name(name: &str, taken: &[String]) -> String {
    let is_free = |candidate: &str| !taken.iter().any(|t| t == candidate);
    if is_free(name) {
        return name.to_string();
    }
    (1..)
        .map(|n| match n {
            1 => format!("{name} (copy)"),
            n => format!("{name} (copy {n})"),
        })
        .find(|candidate| is_free(candidate))
        .expect("an unbounded range always yields a free name")
}

#[cfg(test)]
mod tests {
    use super::free_name;

    fn taken(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn free_name_is_kept_as_is() {
        assert_eq!(free_name("notes", &taken(&["work"])), "notes");
    }

    #[test]
    fn first_collision_gets_copy_suffix() {
        assert_eq!(free_name("notes", &taken(&["notes"])), "notes (copy)");
    }

    #[test]
    fn further_collisions_count_up() {
        assert_eq!(
            free_name("notes", &taken(&["notes", "notes (copy)"])),
            "notes (copy 2)"
        );
        assert_eq!(
            free_name(
                "notes",
                &taken(&["notes", "notes (copy)", "notes (copy 2)"])
            ),
            "notes (copy 3)"
        );
    }
}
