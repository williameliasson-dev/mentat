//! Drives `NotesView` through real key events, so the bindings themselves are
//! covered rather than just the services underneath them.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui::{Database, Services, View, views::NotesView};

fn services() -> Services {
    Services::new(Database::in_memory().unwrap().repositories())
}

fn press(view: &mut NotesView, services: &Services, code: KeyCode) {
    view.handle_events(services, Event::Key(KeyEvent::from(code)));
}

fn press_ctrl(view: &mut NotesView, services: &Services, c: char) {
    view.handle_events(
        services,
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
    );
}

fn titles_in(services: &Services, folder: Option<i64>) -> Vec<String> {
    services
        .notes
        .list_notes(folder)
        .unwrap()
        .into_iter()
        .map(|n| n.title)
        .collect()
}

#[test]
fn space_marks_and_x_p_moves_the_selection() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    s.notes.create_note(None, "alpha", "").unwrap();
    s.notes.create_note(None, "beta", "").unwrap();
    let mut view = NotesView::new(&s);

    // Listing is [work/, note, note] — folders first, then the two notes in
    // whichever order they tie into.
    press(&mut view, &s, KeyCode::Char('j')); // onto the first note
    press(&mut view, &s, KeyCode::Char(' ')); // mark it, cursor falls to the next
    press(&mut view, &s, KeyCode::Char(' ')); // mark that one too
    press(&mut view, &s, KeyCode::Char('x')); // cut both
    press(&mut view, &s, KeyCode::Char('k')); // back up to work/
    press(&mut view, &s, KeyCode::Char('k'));
    press(&mut view, &s, KeyCode::Char('l')); // open work/
    press(&mut view, &s, KeyCode::Char('p')); // paste

    let mut moved = titles_in(&s, Some(work.id));
    moved.sort();
    assert_eq!(moved, vec!["alpha", "beta"]);
    assert!(titles_in(&s, None).is_empty());
}

#[test]
fn x_p_with_no_marks_moves_just_the_cursor_entry() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    s.notes.create_note(None, "alpha", "").unwrap();
    s.notes.create_note(None, "beta", "").unwrap();
    let mut view = NotesView::new(&s);

    // Notes created in the same second tie on `updated_at`, so which one the
    // list shows first isn't fixed — ask, rather than assume.
    let first = titles_in(&s, None)[0].clone();

    press(&mut view, &s, KeyCode::Char('j')); // onto the first note
    press(&mut view, &s, KeyCode::Char('x'));
    press(&mut view, &s, KeyCode::Char('k'));
    press(&mut view, &s, KeyCode::Char('l'));
    press(&mut view, &s, KeyCode::Char('p'));

    assert_eq!(titles_in(&s, Some(work.id)), vec![first.clone()]);
    assert_eq!(titles_in(&s, None).len(), 1);
    assert_ne!(titles_in(&s, None)[0], first);
}

#[test]
fn y_p_copies_and_leaves_the_original() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    s.notes.create_note(None, "alpha", "body").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('j')); // onto alpha
    press(&mut view, &s, KeyCode::Char('y'));
    press(&mut view, &s, KeyCode::Char('k'));
    press(&mut view, &s, KeyCode::Char('l')); // into work/
    press(&mut view, &s, KeyCode::Char('p'));

    assert_eq!(titles_in(&s, Some(work.id)), vec!["alpha"]);
    assert_eq!(titles_in(&s, None), vec!["alpha"]);
}

#[test]
fn a_copy_can_be_pasted_more_than_once() {
    let s = services();
    s.notes.create_note(None, "alpha", "").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('y')); // copy alpha
    press(&mut view, &s, KeyCode::Char('p')); // duplicate in place
    press(&mut view, &s, KeyCode::Char('p')); // and again

    let mut titles = titles_in(&s, None);
    titles.sort();
    // Sorted: the space in "(copy 2)" orders ahead of the ")" in "(copy)".
    assert_eq!(titles, vec!["alpha", "alpha (copy 2)", "alpha (copy)"]);
}

#[test]
fn a_cut_is_consumed_by_its_paste() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    s.notes.create_note(None, "alpha", "").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('j'));
    press(&mut view, &s, KeyCode::Char('x'));
    press(&mut view, &s, KeyCode::Char('k'));
    press(&mut view, &s, KeyCode::Char('l'));
    press(&mut view, &s, KeyCode::Char('p'));
    press(&mut view, &s, KeyCode::Char('p')); // no-op: clipboard is empty

    assert_eq!(titles_in(&s, Some(work.id)), vec!["alpha"]);
}

#[test]
fn ctrl_a_marks_everything_and_d_deletes_it() {
    let s = services();
    s.folders.create_folder(None, "work").unwrap();
    s.notes.create_note(None, "alpha", "").unwrap();
    s.notes.create_note(None, "beta", "").unwrap();
    let mut view = NotesView::new(&s);

    press_ctrl(&mut view, &s, 'a');
    press(&mut view, &s, KeyCode::Char('d'));
    press(&mut view, &s, KeyCode::Char('y')); // confirm

    assert!(titles_in(&s, None).is_empty());
    assert!(s.folders.list_folders(None).unwrap().is_empty());
}

#[test]
fn ctrl_a_twice_clears_the_marks() {
    let s = services();
    s.notes.create_note(None, "alpha", "").unwrap();
    s.notes.create_note(None, "beta", "").unwrap();
    let mut view = NotesView::new(&s);

    press_ctrl(&mut view, &s, 'a');
    press_ctrl(&mut view, &s, 'a');
    press(&mut view, &s, KeyCode::Char('d')); // falls back to the cursor entry
    press(&mut view, &s, KeyCode::Char('y'));

    assert_eq!(titles_in(&s, None).len(), 1);
}

#[test]
fn esc_drops_a_pending_cut() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    s.notes.create_note(None, "alpha", "").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('j'));
    press(&mut view, &s, KeyCode::Char('x'));
    press(&mut view, &s, KeyCode::Esc); // clears the clipboard
    press(&mut view, &s, KeyCode::Char('k'));
    press(&mut view, &s, KeyCode::Char('l'));
    press(&mut view, &s, KeyCode::Char('p'));

    assert!(titles_in(&s, Some(work.id)).is_empty());
    assert_eq!(titles_in(&s, None), vec!["alpha"]);
}

#[test]
fn a_folder_pasted_into_its_own_child_is_refused() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    let child = s.folders.create_folder(Some(work.id), "child").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('x')); // cut work/
    press(&mut view, &s, KeyCode::Char('l')); // into work/
    press(&mut view, &s, KeyCode::Char('l')); // into work/child/
    press(&mut view, &s, KeyCode::Char('p'));

    // Still where it was, with the child intact underneath it.
    assert_eq!(s.folders.get_folder(work.id).unwrap().parent_id, None);
    assert_eq!(
        s.folders.get_folder(child.id).unwrap().parent_id,
        Some(work.id)
    );
}

/// The whole screen as text, with styling dropped.
fn screen(view: &NotesView) -> String {
    let backend = ratatui::backend::TestBackend::new(90, 20);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|frame| view.render(frame)).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(90)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn marking_shows_a_tick_and_a_count() {
    let s = services();
    s.notes.create_note(None, "alpha", "").unwrap();
    let mut view = NotesView::new(&s);

    assert!(!screen(&view).contains("marked"));

    press(&mut view, &s, KeyCode::Char(' '));

    let screen = screen(&view);
    assert!(
        screen.contains("\u{f00c}"),
        "no tick in the gutter:\n{screen}"
    );
    assert!(
        screen.contains("1 marked"),
        "no count in the footer:\n{screen}"
    );
}

#[test]
fn a_marked_row_is_tinted_across_its_width() {
    let s = services();
    s.notes.create_note(None, "alpha", "").unwrap();
    s.notes.create_note(None, "beta", "").unwrap();
    let mut view = NotesView::new(&s);

    // Marking steps the cursor onto the next row, so the marked row is left
    // showing its own colour rather than the cursor's.
    press(&mut view, &s, KeyCode::Char(' '));

    let backend = ratatui::backend::TestBackend::new(90, 20);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|frame| view.render(frame)).unwrap();
    let buffer = terminal.backend().buffer().clone();

    // Row 0 is the title bar, row 1 the pane border, so row 2 is the first
    // entry. Sample the label and well past the end of the text.
    let marked = ratatui::style::Color::Rgb(58, 48, 33);
    assert_eq!(buffer[(4, 2)].bg, marked, "label not tinted");
    assert_eq!(
        buffer[(25, 2)].bg,
        marked,
        "tint stops short of the row end"
    );
    // The unmarked row below it stays clean.
    assert_ne!(buffer[(25, 3)].bg, marked);
}

#[test]
fn a_pending_cut_is_reported_in_the_footer() {
    let s = services();
    s.notes.create_note(None, "alpha", "").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('x'));
    assert!(screen(&view).contains("1 cut"));

    // The status message from the paste replaces the hints.
    press(&mut view, &s, KeyCode::Char('p'));
    assert!(screen(&view).contains("moved 1"));
}

#[test]
fn the_delete_confirmation_names_what_it_will_take() {
    let s = services();
    s.notes.create_note(None, "alpha", "").unwrap();
    s.notes.create_note(None, "beta", "").unwrap();
    let mut view = NotesView::new(&s);

    press_ctrl(&mut view, &s, 'a');
    press(&mut view, &s, KeyCode::Char('d'));

    let screen = screen(&view);
    assert!(screen.contains("Delete these 2?"), "{screen}");
    assert!(
        screen.contains("alpha") && screen.contains("beta"),
        "{screen}"
    );
}

fn type_text(view: &mut NotesView, services: &Services, text: &str) {
    for c in text.chars() {
        press(view, services, KeyCode::Char(c));
    }
}

#[test]
fn r_renames_a_note() {
    let s = services();
    let note = s.notes.create_note(None, "alpha", "body").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('r'));
    // The input starts preloaded with the old name, cursor at the end.
    type_text(&mut view, &s, " reborn");
    press(&mut view, &s, KeyCode::Enter);

    let renamed = s.notes.get_note(note.id).unwrap();
    assert_eq!(renamed.title, "alpha reborn");
    // Renaming a note must not disturb what's in it.
    assert_eq!(renamed.body, "body");
}

#[test]
fn r_renames_a_folder() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('r'));
    press(&mut view, &s, KeyCode::Backspace); // "wor"
    press(&mut view, &s, KeyCode::Enter);

    assert_eq!(s.folders.get_folder(work.id).unwrap().name, "wor");
}

#[test]
fn esc_abandons_a_rename() {
    let s = services();
    let note = s.notes.create_note(None, "alpha", "").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('r'));
    type_text(&mut view, &s, "!!!");
    press(&mut view, &s, KeyCode::Esc);

    assert_eq!(s.notes.get_note(note.id).unwrap().title, "alpha");
}

#[test]
fn renaming_a_folder_onto_a_siblings_name_is_refused() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    s.folders.create_folder(None, "notes").unwrap();
    let mut view = NotesView::new(&s);

    // Listing is [notes/, work/] — folders sort by name.
    press(&mut view, &s, KeyCode::Char('j')); // onto work/
    press(&mut view, &s, KeyCode::Char('r'));
    for _ in 0..4 {
        press(&mut view, &s, KeyCode::Backspace);
    }
    type_text(&mut view, &s, "notes");
    press(&mut view, &s, KeyCode::Enter);

    assert_eq!(s.folders.get_folder(work.id).unwrap().name, "work");
    assert!(screen(&view).contains("already exists there"));
}

#[test]
fn an_empty_rename_is_not_committed() {
    let s = services();
    let note = s.notes.create_note(None, "alpha", "").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('r'));
    for _ in 0..5 {
        press(&mut view, &s, KeyCode::Backspace);
    }
    press(&mut view, &s, KeyCode::Enter); // ignored — still in the input
    type_text(&mut view, &s, "renamed");
    press(&mut view, &s, KeyCode::Enter);

    assert_eq!(s.notes.get_note(note.id).unwrap().title, "renamed");
}

#[test]
fn marks_survive_a_folder_change() {
    let s = services();
    let work = s.folders.create_folder(None, "work").unwrap();
    s.notes.create_note(Some(work.id), "buried", "").unwrap();
    s.notes.create_note(None, "loose", "").unwrap();
    let mut view = NotesView::new(&s);

    press(&mut view, &s, KeyCode::Char('j')); // onto loose
    press(&mut view, &s, KeyCode::Char(' ')); // mark it
    press(&mut view, &s, KeyCode::Char('k')); // back to work/
    press(&mut view, &s, KeyCode::Char('l')); // open work/
    press(&mut view, &s, KeyCode::Char(' ')); // mark buried too
    press(&mut view, &s, KeyCode::Char('d'));
    press(&mut view, &s, KeyCode::Char('y'));

    assert!(titles_in(&s, None).is_empty());
    assert!(titles_in(&s, Some(work.id)).is_empty());
}
