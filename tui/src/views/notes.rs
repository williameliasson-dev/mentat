use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{Action, Folder, Note, Services, View, consts::colors, service::transfer};

const TITLE_MAX: usize = 60;

// Might wanna configurable down the road due to nerd font
const FOLDER_ICON: &str = "\u{f07b}"; // nf-fa-folder
const NOTE_ICON: &str = "\u{f15c}"; // nf-fa-file_text

/// Gutter marker for a selected entry.
const MARK: &str = "\u{f00c} "; // nf-fa-check

#[derive(PartialEq)]
enum Mode {
    Navigate,
    Create,
    Rename,
    ConfirmDelete,
}

impl Mode {
    /// Whether the input line is live — it owns the keyboard and the cursor.
    fn is_input(&self) -> bool {
        matches!(self, Mode::Create | Mode::Rename)
    }
}

/// Identifies an entry across reloads and across folders. Note and folder ids
/// come from different tables and overlap, so the kind has to travel with it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryId {
    Folder(i64),
    Note(i64),
}

/// What `p` will do with the clipboard.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Cut,
    Copy,
}

/// Entries waiting on a paste, and what to do with them.
struct Clipboard {
    op: Op,
    items: Vec<EntryId>,
}

/// One row in a listing. Folders sort ahead of notes.
enum Entry {
    Folder(Folder),
    Note(Note),
}

impl Entry {
    fn id(&self) -> EntryId {
        match self {
            Entry::Folder(f) => EntryId::Folder(f.id),
            Entry::Note(n) => EntryId::Note(n.id),
        }
    }

    fn label(&self) -> String {
        match self {
            Entry::Folder(f) => format!("{FOLDER_ICON} {}/", f.name),
            Entry::Note(n) => format!("{NOTE_ICON} {}", n.title),
        }
    }

    fn style(&self) -> Style {
        match self {
            Entry::Folder(_) => Style::new().fg(colors::SAND).bold(),
            Entry::Note(_) => Style::new().fg(colors::IBAD),
        }
    }

    /// Accent for the preview pane's border and title.
    fn accent(&self) -> Color {
        match self {
            Entry::Folder(_) => colors::SAND,
            Entry::Note(_) => colors::IBAD,
        }
    }
}

/// What `input` will produce on submit. A leading or trailing `/` means folder,
/// as in yazi — `/notes` and `notes/` both create a folder called `notes`.
enum Create<'a> {
    Nothing,
    Folder(&'a str),
    Note(&'a str),
}

fn parse_create(input: &str) -> Create<'_> {
    let raw = input.trim();
    let is_folder = raw.starts_with('/') || raw.ends_with('/');
    let name = raw.trim_matches('/').trim();
    if name.is_empty() {
        Create::Nothing
    } else if is_folder {
        Create::Folder(name)
    } else {
        Create::Note(name)
    }
}

pub struct NotesView {
    /// Breadcrumb from the root. Empty means we're at the root; the last
    /// element is the folder currently open, so `pop` is "go up".
    path: Vec<Folder>,
    entries: Vec<Entry>,
    /// Children of the highlighted folder, for the preview pane. Empty when a
    /// note is highlighted. Refreshed on every selection change, since `render`
    /// has no access to `Services`.
    peek: Vec<Entry>,
    list_state: ListState,
    mode: Mode,
    input: String,
    /// Byte cursor within `input`.
    input_cursor: usize,
    /// Marked entries, in the order they were marked. Kept across folder
    /// changes so a selection can be gathered from several places before
    /// acting on it. A `Vec` rather than a set: selections are small, and the
    /// order decides the order operations report in.
    marks: Vec<EntryId>,
    /// Populated by `x`/`y`, drained by `p`.
    clipboard: Option<Clipboard>,
    /// Footer message for the last action; cleared by the next keypress.
    status: Option<String>,
}

impl NotesView {
    pub fn new(services: &Services) -> Self {
        let mut view = Self {
            path: Vec::new(),
            entries: Vec::new(),
            peek: Vec::new(),
            list_state: ListState::default(),
            mode: Mode::Navigate,
            input: String::new(),
            input_cursor: 0,
            marks: Vec::new(),
            clipboard: None,
            status: None,
        };
        view.reload(services);
        view
    }

    /// Id of the folder currently open; `None` at the root.
    fn folder_id(&self) -> Option<i64> {
        self.path.last().map(|f| f.id)
    }

    /// `/foo/bar/` for the title bar.
    fn breadcrumb(&self) -> String {
        let mut s = String::from(" /");
        for folder in &self.path {
            s.push_str(&folder.name);
            s.push('/');
        }
        s.push(' ');
        s
    }

    fn children_of(services: &Services, folder_id: Option<i64>) -> Vec<Entry> {
        let folders = services.folders.list_folders(folder_id).unwrap_or_default();
        let notes = services.notes.list_notes(folder_id).unwrap_or_default();
        folders
            .into_iter()
            .map(Entry::Folder)
            .chain(notes.into_iter().map(Entry::Note))
            .collect()
    }

    fn reload(&mut self, services: &Services) {
        self.entries = Self::children_of(services, self.folder_id());

        if self.entries.is_empty() {
            self.list_state.select(None);
        } else {
            let i = self
                .list_state
                .selected()
                .unwrap_or(0)
                .min(self.entries.len() - 1);
            self.list_state.select(Some(i));
        }
        self.refresh_peek(services);
    }

    /// Loads the highlighted folder's contents for the preview pane.
    fn refresh_peek(&mut self, services: &Services) {
        let folder_id = match self.selected() {
            Some(Entry::Folder(f)) => Some(f.id),
            _ => None,
        };
        self.peek = match folder_id {
            Some(id) => Self::children_of(services, Some(id)),
            None => Vec::new(),
        };
    }

    fn select_next(&mut self, services: &Services) {
        if self.entries.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((i + 1).min(self.entries.len() - 1)));
        self.refresh_peek(services);
    }

    fn select_previous(&mut self, services: &Services) {
        if self.entries.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(i.saturating_sub(1)));
        self.refresh_peek(services);
    }

    fn selected(&self) -> Option<&Entry> {
        self.list_state.selected().and_then(|i| self.entries.get(i))
    }

    fn open_selected_folder(&mut self, services: &Services) {
        let Some(Entry::Folder(folder)) = self.selected() else {
            return;
        };
        self.path.push(folder.clone());
        self.list_state.select(Some(0));
        self.reload(services);
    }

    fn go_up(&mut self, services: &Services) {
        if self.path.pop().is_some() {
            self.list_state.select(Some(0));
            self.reload(services);
        }
    }

    // -- Selection ------------------------------------------------------------

    /// What an action applies to: everything marked, or the entry under the
    /// cursor when nothing is marked.
    fn targets(&self) -> Vec<EntryId> {
        if self.marks.is_empty() {
            self.selected().map(Entry::id).into_iter().collect()
        } else {
            self.marks.clone()
        }
    }

    /// Whether this entry is staged for a move — it renders dimmed until the
    /// paste happens.
    fn is_cut(&self, id: EntryId) -> bool {
        self.clipboard
            .as_ref()
            .is_some_and(|c| c.op == Op::Cut && c.items.contains(&id))
    }

    fn toggle_mark(&mut self, services: &Services) {
        let Some(id) = self.selected().map(Entry::id) else {
            return;
        };
        match self.marks.iter().position(|m| *m == id) {
            Some(i) => {
                self.marks.remove(i);
            }
            None => self.marks.push(id),
        }
        // Marking walks down the list, so a run can be marked with repeated
        // taps of space — as in yazi.
        self.select_next(services);
    }

    /// Marks everything here, or clears the marks if it's all marked already.
    fn toggle_mark_all(&mut self) {
        let here: Vec<EntryId> = self.entries.iter().map(Entry::id).collect();
        if here.iter().all(|id| self.marks.contains(id)) {
            self.marks.retain(|id| !here.contains(id));
        } else {
            for id in here {
                if !self.marks.contains(&id) {
                    self.marks.push(id);
                }
            }
        }
        self.status = Some(format!("{} marked", self.marks.len()));
    }

    fn clear_selection(&mut self) {
        self.marks.clear();
        self.clipboard = None;
    }

    /// `x` and `y`: load the targets into the clipboard.
    fn stage(&mut self, op: Op) {
        let items = self.targets();
        if items.is_empty() {
            return;
        }
        self.status = Some(format!(
            "{} {}",
            items.len(),
            match op {
                Op::Cut => "cut",
                Op::Copy => "copied",
            }
        ));
        self.clipboard = Some(Clipboard { op, items });
        // The clipboard now carries the selection; cut entries show dimmed and
        // copied ones need no marker, so leaving ticks behind only confuses.
        self.marks.clear();
    }

    /// `p`: drop the clipboard into the folder currently open.
    ///
    /// A cut is consumed by its paste; a copy stays on the clipboard so it can
    /// be pasted into several places.
    fn paste(&mut self, services: &Services) {
        let Some(clipboard) = self.clipboard.take() else {
            self.status = Some("nothing to paste".to_string());
            return;
        };
        let dest = self.folder_id();

        let mut done = 0usize;
        let mut failure = None;
        for id in &clipboard.items {
            let result = match (clipboard.op, id) {
                (Op::Cut, EntryId::Note(id)) => services.notes.move_note(*id, dest).map(drop),
                (Op::Cut, EntryId::Folder(id)) => services.folders.move_folder(*id, dest).map(drop),
                (Op::Copy, EntryId::Note(id)) => services
                    .notes
                    .get_note(*id)
                    .and_then(|n| transfer::copy_note(services, &n, dest))
                    .map(drop),
                (Op::Copy, EntryId::Folder(id)) => services
                    .folders
                    .get_folder(*id)
                    .and_then(|f| transfer::copy_folder(services, &f, dest))
                    .map(drop),
            };
            match result {
                Ok(()) => done += 1,
                // First failure is the one worth reading; the rest are usually
                // the same cause repeated.
                Err(e) => failure = failure.or(Some(e.to_string())),
            }
        }

        let verb = match clipboard.op {
            Op::Cut => "moved",
            Op::Copy => "copied",
        };
        self.status = Some(match failure {
            Some(reason) => format!(
                "{verb} {done}, {} failed — {reason}",
                clipboard.items.len() - done
            ),
            None => format!("{verb} {done}"),
        });

        if clipboard.op == Op::Copy {
            self.clipboard = Some(clipboard);
        }
        self.reload(services);
    }

    fn delete_targets(&mut self, services: &Services) {
        let mut failure = None;
        for id in self.targets() {
            let result = match id {
                EntryId::Folder(id) => services.folders.delete_folder(id),
                EntryId::Note(id) => services.notes.delete_note(id),
            };
            if let Err(e) = result {
                failure = failure.or(Some(e.to_string()));
            }
        }
        self.status = failure.map(|reason| format!("delete failed — {reason}"));
        // Deleted ids must not linger in either buffer: a later paste would
        // chase rows that no longer exist.
        self.clear_selection();
        self.reload(services);
    }

    fn instructions(&self) -> Line<'static> {
        let key = |s: &'static str| Span::styled(s, Style::new().fg(colors::SAND).bold());
        let text = |s: &'static str| Span::styled(s, Style::new().fg(colors::DIM));
        match self.mode {
            // A message about the last action displaces the hints — it's the
            // only feedback a paste or a failed move gets.
            Mode::Navigate => match &self.status {
                Some(message) => Line::from(vec![
                    Span::styled(" ▪ ", Style::new().fg(colors::SAND)),
                    Span::styled(message.clone(), Style::new().fg(colors::TEXT)),
                ]),
                None => {
                    let mut spans = vec![
                        text(" Nav "),
                        key("<hjkl>"),
                        text(" Mark "),
                        key("<space>"),
                        text(" Cut "),
                        key("<x>"),
                        text(" Copy "),
                        key("<y>"),
                        text(" Paste "),
                        key("<p>"),
                        text(" Add "),
                        key("<a>"),
                        text(" Rename "),
                        key("<r>"),
                        text(" Del "),
                        key("<d>"),
                    ];
                    // Badges last: they only appear mid-selection, and the
                    // hints shouldn't jump sideways when they do.
                    if !self.marks.is_empty() {
                        spans.push(Span::styled(
                            format!("  {} marked", self.marks.len()),
                            Style::new().fg(colors::SAND).bold(),
                        ));
                    }
                    if let Some(clipboard) = &self.clipboard {
                        spans.push(Span::styled(
                            format!(
                                "  {} {}",
                                clipboard.items.len(),
                                match clipboard.op {
                                    Op::Cut => "cut",
                                    Op::Copy => "copied",
                                }
                            ),
                            Style::new().fg(colors::IBAD).bold(),
                        ));
                    }
                    Line::from(spans)
                }
            },
            Mode::Create => Line::from(vec![
                text(" Create "),
                key("<Enter>"),
                text(" Cancel "),
                key("<Esc>"),
                text("   trailing "),
                key("/"),
                text(" makes a folder"),
            ]),
            Mode::Rename => Line::from(vec![
                text(" Rename "),
                key("<Enter>"),
                text(" Cancel "),
                key("<Esc>"),
            ]),
            Mode::ConfirmDelete => Line::from(vec![
                text(" Delete? "),
                Span::styled("<y>", Style::new().fg(colors::DANGER).bold()),
                text(" / "),
                key("<n>"),
            ]),
        }
    }

    fn handle_navigate(&mut self, services: &Services, key: KeyEvent) -> Option<Action> {
        // Last action's message has been read by now.
        self.status = None;

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char('a') = key.code {
                self.toggle_mark_all();
            }
            return None;
        }

        match key.code {
            KeyCode::Char('q') => Some(Action::Exit),
            // Esc backs out of a selection first; only an idle Esc leaves.
            KeyCode::Esc => {
                if self.marks.is_empty() && self.clipboard.is_none() {
                    return Some(Action::SwitchTo(Box::new(crate::views::HomeView::new())));
                }
                self.clear_selection();
                None
            }
            KeyCode::Char(' ') => {
                self.toggle_mark(services);
                None
            }
            KeyCode::Char('x') => {
                self.stage(Op::Cut);
                None
            }
            KeyCode::Char('y') => {
                self.stage(Op::Copy);
                None
            }
            KeyCode::Char('p') => {
                self.paste(services);
                None
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next(services);
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_previous(services);
                None
            }
            KeyCode::Char('a') => {
                self.mode = Mode::Create;
                self.input.clear();
                self.input_cursor = 0;
                None
            }
            KeyCode::Char('r') => {
                self.begin_rename();
                None
            }
            KeyCode::Char('h') | KeyCode::Char('-') | KeyCode::Backspace | KeyCode::Left => {
                self.go_up(services);
                None
            }
            KeyCode::Char('l') | KeyCode::Enter | KeyCode::Right => match self.selected() {
                Some(Entry::Folder(_)) => {
                    self.open_selected_folder(services);
                    None
                }
                Some(Entry::Note(n)) => Some(Action::EditNote(n.id)),
                None => None,
            },
            KeyCode::Char('d') => {
                if !self.targets().is_empty() {
                    self.mode = Mode::ConfirmDelete;
                }
                None
            }
            _ => None,
        }
    }

    fn handle_create(&mut self, services: &Services, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Navigate,
            KeyCode::Enter => {
                let here = self.folder_id();
                match parse_create(&self.input) {
                    Create::Nothing => return None,
                    Create::Folder(name) => {
                        let _ = services.folders.create_folder(here, name);
                    }
                    Create::Note(title) => {
                        let _ = services.notes.create_note(here, title, "");
                    }
                }
                self.reload(services);
                self.mode = Mode::Navigate;
            }
            _ => self.edit_input(key),
        }
        None
    }

    /// Opens the input line preloaded with the current name, cursor at the end
    /// — a rename is usually a tweak, not a retype.
    fn begin_rename(&mut self) {
        let Some(entry) = self.selected() else { return };
        self.input = match entry {
            Entry::Folder(f) => f.name.clone(),
            Entry::Note(n) => n.title.clone(),
        };
        self.input_cursor = self.input.len();
        self.mode = Mode::Rename;
    }

    fn handle_rename(&mut self, services: &Services, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Navigate,
            KeyCode::Enter => {
                let name = self.input.trim().to_string();
                // An empty name would leave an unclickable row; stay in the
                // input instead of committing it.
                if name.is_empty() {
                    return None;
                }
                let result = match self.selected() {
                    Some(Entry::Folder(f)) => services.folders.rename_folder(f.id, &name).map(drop),
                    Some(Entry::Note(n)) => {
                        services.notes.update_note(n.id, &name, &n.body).map(drop)
                    }
                    None => Ok(()),
                };
                if let Err(e) = result {
                    self.status = Some(format!("rename failed — {e}"));
                }
                self.reload(services);
                self.mode = Mode::Navigate;
            }
            _ => self.edit_input(key),
        }
        None
    }

    fn handle_confirm_delete(&mut self, services: &Services, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('y') => {
                self.delete_targets(services);
                self.mode = Mode::Navigate;
            }
            KeyCode::Char('n') | KeyCode::Esc => self.mode = Mode::Navigate,
            _ => {}
        }
        None
    }

    /// Single-line input editing at `input_cursor` over `input`.
    fn edit_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                if self.input.len() < TITLE_MAX {
                    self.input.insert(self.input_cursor, c);
                    self.input_cursor += c.len_utf8();
                }
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    let prev = prev_char_boundary(&self.input, self.input_cursor);
                    self.input.replace_range(prev..self.input_cursor, "");
                    self.input_cursor = prev;
                }
            }
            KeyCode::Delete => {
                if self.input_cursor < self.input.len() {
                    let next = next_char_boundary(&self.input, self.input_cursor);
                    self.input.replace_range(self.input_cursor..next, "");
                }
            }
            KeyCode::Left => self.input_cursor = prev_char_boundary(&self.input, self.input_cursor),
            KeyCode::Right => {
                self.input_cursor = next_char_boundary(&self.input, self.input_cursor)
            }
            KeyCode::Home => self.input_cursor = 0,
            KeyCode::End => self.input_cursor = self.input.len(),
            _ => {}
        }
    }

    /// The confirmation pane — always lists what's going, since `d` can now
    /// take a selection built up across several folders.
    fn confirm_delete_lines(&self) -> Vec<Line<'static>> {
        const LISTED: usize = 12;

        let targets = self.targets();
        let mut lines = vec![
            Line::from(Span::styled(
                match targets.len() {
                    1 => "Delete this?".to_string(),
                    n => format!("Delete these {n}?"),
                },
                Style::new().fg(colors::DANGER).bold(),
            )),
            Line::from(Span::styled(
                "Folders take everything inside them.",
                Style::new().fg(colors::DIM),
            )),
            Line::from(""),
        ];

        for id in targets.iter().take(LISTED) {
            // A mark made in another folder isn't in `entries` to name, but
            // it's still going — say so rather than silently omitting it.
            let entry = self.entries.iter().find(|e| e.id() == *id);
            lines.push(match entry {
                Some(e) => Line::from(Span::styled(e.label(), e.style())),
                None => Line::from(Span::styled(
                    "marked in another folder",
                    Style::new().fg(colors::DIM).italic(),
                )),
            });
        }
        if targets.len() > LISTED {
            lines.push(Line::from(Span::styled(
                format!("… and {} more", targets.len() - LISTED),
                Style::new().fg(colors::DIM),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("<y>", Style::new().fg(colors::DANGER).bold()),
            Span::styled(" delete   ", Style::new().fg(colors::DIM)),
            Span::styled("<n>", Style::new().fg(colors::SAND).bold()),
            Span::styled(" cancel", Style::new().fg(colors::DIM)),
        ]));
        lines
    }

    /// The preview pane: title, contents, and the accent that tells a note
    /// apart from a folder without reading a word of it.
    /// `width` is the pane's inner width, so the header rule spans it exactly
    /// instead of wrapping onto a second line.
    fn preview(&self, width: usize) -> (String, Vec<Line<'static>>, Color) {
        let dim = |s: String| Line::from(Span::styled(s, Style::new().fg(colors::DIM)));
        let rule = |color| rule(color, width);

        match self.selected() {
            Some(Entry::Note(n)) => {
                let words = n.body.split_whitespace().count();
                let mut lines = vec![
                    dim(format!(
                        "note · {words} words · edited {}",
                        relative(n.updated_at)
                    )),
                    rule(colors::IBAD),
                ];
                if n.body.trim().is_empty() {
                    lines.push(Line::from(Span::styled(
                        "empty — <enter> to write",
                        Style::new().fg(colors::DIM).italic(),
                    )));
                } else {
                    lines.extend(crate::markdown::render(&n.body));
                }
                (format!(" {NOTE_ICON} {} ", n.title), lines, colors::IBAD)
            }
            Some(Entry::Folder(f)) => {
                let folders = self
                    .peek
                    .iter()
                    .filter(|e| matches!(e, Entry::Folder(_)))
                    .count();
                let notes = self.peek.len() - folders;
                let mut lines = vec![
                    dim(format!("folder · {folders} folders · {notes} notes")),
                    rule(colors::SAND),
                ];
                if self.peek.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "empty",
                        Style::new().fg(colors::DIM).italic(),
                    )));
                } else {
                    lines.extend(
                        self.peek
                            .iter()
                            .map(|e| Line::from(Span::styled(e.label(), e.style()))),
                    );
                }
                (format!(" {FOLDER_ICON} {}/ ", f.name), lines, colors::SAND)
            }
            None => (
                " / ".to_string(),
                vec![Line::from(Span::styled(
                    "Empty. <a> to add — end with / for a folder.",
                    Style::new().fg(colors::DIM),
                ))],
                colors::DIM,
            ),
        }
    }
}

impl View for NotesView {
    fn handle_events(&mut self, services: &Services, event: Event) -> Option<Action> {
        let Event::Key(key) = event else { return None };
        match self.mode {
            Mode::Navigate => self.handle_navigate(services, key),
            Mode::Create => self.handle_create(services, key),
            Mode::Rename => self.handle_rename(services, key),
            Mode::ConfirmDelete => self.handle_confirm_delete(services, key),
        }
    }

    fn render(&self, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

        frame.render_widget(
            Paragraph::new(" mentat").style(Style::new().fg(colors::SAND).bold()),
            layout[0],
        );

        let panes = Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(layout[1]);

        let pane_block = |title: &str, accent: Color| {
            Block::new()
                .title(Span::styled(
                    title.to_string(),
                    Style::new().fg(accent).bold(),
                ))
                .borders(Borders::ALL)
                .border_style(Style::new().fg(colors::DIM))
        };

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|e| {
                let id = e.id();
                let mut style = e.style();
                // Cut entries are still here until the paste lands; dimming
                // them shows they're in flight.
                if self.is_cut(id) {
                    style = style.add_modifier(Modifier::DIM | Modifier::ITALIC);
                }
                let marked = self.marks.contains(&id);
                let tick = if marked {
                    Span::styled(MARK, Style::new().fg(colors::SAND).bold())
                } else {
                    // Same width as the tick, so labels stay in one column.
                    Span::raw(" ".repeat(MARK.chars().count()))
                };
                let row = ListItem::new(Line::from(vec![tick, Span::styled(e.label(), style)]));
                // Set on the item rather than the spans so the tint runs the
                // full width of the row, not just under the text.
                if marked {
                    row.style(Style::new().bg(colors::MARKED))
                } else {
                    row
                }
            })
            .collect();
        // The cursor takes the kind's color, so hovering a note never looks
        // like hovering a folder.
        let cursor = self.selected().map_or(colors::SAND, Entry::accent);
        let list = List::new(items)
            .block(pane_block(&self.breadcrumb(), colors::SAND))
            .highlight_style(Style::new().fg(Color::Black).bg(cursor).bold())
            .highlight_symbol("> ");
        let mut state = self.list_state;
        frame.render_stateful_widget(list, panes[0], &mut state);

        match self.mode {
            Mode::Create | Mode::Rename => {
                let title = match self.mode {
                    Mode::Rename => " Rename ",
                    _ => " New name ",
                };
                let p = Paragraph::new(self.input.as_str())
                    .style(Style::new().fg(colors::TEXT))
                    .block(pane_block(title, colors::SAND));
                frame.render_widget(p, panes[1]);
            }
            Mode::ConfirmDelete => {
                frame.render_widget(
                    Paragraph::new(self.confirm_delete_lines())
                        .block(pane_block(" Confirm ", colors::DANGER))
                        .wrap(Wrap { trim: false }),
                    panes[1],
                );
            }
            Mode::Navigate => {
                let inner_width = panes[1].width.saturating_sub(2) as usize;
                let (title, lines, accent) = self.preview(inner_width);
                frame.render_widget(
                    Paragraph::new(lines)
                        .block(pane_block(&title, accent))
                        .wrap(Wrap { trim: false }),
                    panes[1],
                );
            }
        }

        frame.render_widget(self.instructions(), layout[2]);
    }

    fn cursor_position(&self) -> Option<Position> {
        if !self.mode.is_input() {
            return None;
        }
        // Recompute the same layout used in `render` — cheap and stateless.
        let (w, h) = crossterm::terminal::size().ok()?;
        let layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(ratatui::layout::Rect::new(0, 0, w, h));
        let panes = Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(layout[1]);
        let inner = panes[1].inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });
        // `input_cursor` is a byte offset; the terminal wants columns, and a
        // renamed entry can easily carry non-ASCII in its name.
        let column = self.input[..self.input_cursor].chars().count() as u16;
        Some(Position::new(inner.x + column, inner.y))
    }

    fn note_body(&self, id: i64) -> Option<String> {
        self.entries.iter().find_map(|e| match e {
            Entry::Note(n) if n.id == id => Some(n.body.clone()),
            _ => None,
        })
    }

    fn set_note_body(&mut self, services: &Services, id: i64, body: &str) {
        let title = self.entries.iter().find_map(|e| match e {
            Entry::Note(n) if n.id == id => Some(n.title.clone()),
            _ => None,
        });
        let Some(title) = title else { return };
        let _ = services.notes.update_note(id, &title, body);
        self.reload(services);
    }
}

/// Separator under the preview header, in the pane's accent.
fn rule(color: Color, width: usize) -> Line<'static> {
    Line::from(Span::styled("─".repeat(width), Style::new().fg(color)))
}

/// Coarse "how long ago" for the preview header — precision past days isn't
/// worth a date-formatting dependency.
fn relative(ts: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(ts);
    match (now - ts).max(0) {
        s if s < 60 => "just now".to_string(),
        s if s < 3600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3600),
        s => format!("{}d ago", s / 86_400),
    }
}

// -- Cursor helpers (byte-index safe) ----------------------------------------

fn prev_char_boundary(s: &str, i: usize) -> usize {
    if i == 0 {
        return 0;
    }
    let mut j = i - 1;
    while !s.is_char_boundary(j) {
        j -= 1;
    }
    j
}

fn next_char_boundary(s: &str, i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    let mut j = i + 1;
    while j < s.len() && !s.is_char_boundary(j) {
        j += 1;
    }
    j
}

#[cfg(test)]
mod tests {
    use super::{Create, parse_create};

    fn folder(input: &str) -> Option<String> {
        match parse_create(input) {
            Create::Folder(name) => Some(name.to_string()),
            _ => None,
        }
    }

    fn note(input: &str) -> Option<String> {
        match parse_create(input) {
            Create::Note(title) => Some(title.to_string()),
            _ => None,
        }
    }

    #[test]
    fn leading_slash_makes_a_folder() {
        assert_eq!(folder("/notes").as_deref(), Some("notes"));
    }

    #[test]
    fn trailing_slash_makes_a_folder() {
        assert_eq!(folder("notes/").as_deref(), Some("notes"));
    }

    #[test]
    fn plain_name_makes_a_note() {
        assert_eq!(note("shopping list").as_deref(), Some("shopping list"));
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(folder("  /work  ").as_deref(), Some("work"));
        assert_eq!(note("  todo  ").as_deref(), Some("todo"));
    }

    #[test]
    fn slash_only_creates_nothing() {
        assert!(matches!(parse_create("/"), Create::Nothing));
        assert!(matches!(parse_create("//"), Create::Nothing));
        assert!(matches!(parse_create("   "), Create::Nothing));
        assert!(matches!(parse_create(""), Create::Nothing));
    }
}
