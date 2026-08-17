use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{Action, Folder, Note, Services, View, consts::colors};

const TITLE_MAX: usize = 60;

#[derive(PartialEq)]
enum Mode {
    Navigate,
    Create,
    ConfirmDelete,
}

/// One row in a listing. Folders sort ahead of notes.
enum Entry {
    Folder(Folder),
    Note(Note),
}

impl Entry {
    fn label(&self) -> String {
        match self {
            Entry::Folder(f) => format!("{}/", f.name),
            Entry::Note(n) => n.title.clone(),
        }
    }

    fn style(&self) -> Style {
        match self {
            Entry::Folder(_) => Style::new().fg(colors::SAND).bold(),
            Entry::Note(_) => Style::new().fg(colors::TEXT),
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

    fn instructions(&self) -> Line<'static> {
        let key = |s: &'static str| Span::styled(s, Style::new().fg(colors::SAND).bold());
        let text = |s: &'static str| Span::styled(s, Style::new().fg(colors::DIM));
        match self.mode {
            Mode::Navigate => Line::from(vec![
                text(" Add "),
                key("<a>"),
                text(" Open "),
                key("<enter>"),
                text(" Up "),
                key("<->"),
                text(" Delete "),
                key("<d>"),
                text(" Home "),
                key("<h>"),
                text(" Quit "),
                key("<q>"),
            ]),
            Mode::Create => Line::from(vec![
                text(" Create "),
                key("<Enter>"),
                text(" Cancel "),
                key("<Esc>"),
                text("   trailing "),
                key("/"),
                text(" makes a folder"),
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
        match key.code {
            KeyCode::Char('q') => Some(Action::Exit),
            KeyCode::Char('h') => Some(Action::SwitchTo(Box::new(crate::views::HomeView::new()))),
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
            KeyCode::Char('-') | KeyCode::Backspace | KeyCode::Left => {
                self.go_up(services);
                None
            }
            KeyCode::Enter | KeyCode::Right => match self.selected() {
                Some(Entry::Folder(_)) => {
                    self.open_selected_folder(services);
                    None
                }
                Some(Entry::Note(n)) => Some(Action::EditNote(n.id)),
                None => None,
            },
            KeyCode::Char('d') => {
                if self.selected().is_some() {
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

    fn handle_confirm_delete(&mut self, services: &Services, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('y') => {
                match self.selected() {
                    Some(Entry::Folder(f)) => {
                        let _ = services.folders.delete_folder(f.id);
                    }
                    Some(Entry::Note(n)) => {
                        let _ = services.notes.delete_note(n.id);
                    }
                    None => {}
                }
                self.reload(services);
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

    /// The preview pane: a note's rendered body, or a folder's contents.
    fn preview(&self) -> (String, Paragraph<'_>) {
        match self.selected() {
            Some(Entry::Note(n)) => (
                format!(" {} ", n.title),
                Paragraph::new(crate::markdown::render(&n.body)),
            ),
            Some(Entry::Folder(f)) => {
                let title = format!(" {}/ ", f.name);
                if self.peek.is_empty() {
                    return (
                        title,
                        Paragraph::new("empty").style(Style::new().fg(colors::DIM)),
                    );
                }
                let lines: Vec<Line> = self
                    .peek
                    .iter()
                    .map(|e| Line::from(Span::styled(e.label(), e.style())))
                    .collect();
                (title, Paragraph::new(lines))
            }
            None => (
                " / ".to_string(),
                Paragraph::new("Empty. <a> to add — end with / for a folder.")
                    .style(Style::new().fg(colors::DIM)),
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

        let pane_block = |title: &str| {
            Block::new()
                .title(Span::styled(
                    title.to_string(),
                    Style::new().fg(colors::SAND),
                ))
                .borders(Borders::ALL)
                .border_style(Style::new().fg(colors::DIM))
        };

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|e| ListItem::new(e.label()).style(e.style()))
            .collect();
        let list = List::new(items)
            .block(pane_block(&self.breadcrumb()))
            .highlight_style(Style::new().fg(Color::Black).bg(colors::SAND).bold())
            .highlight_symbol("> ");
        let mut state = self.list_state;
        frame.render_stateful_widget(list, panes[0], &mut state);

        match self.mode {
            Mode::Create => {
                let p = Paragraph::new(self.input.as_str())
                    .style(Style::new().fg(colors::TEXT))
                    .block(pane_block(" New name "));
                frame.render_widget(p, panes[1]);
            }
            Mode::ConfirmDelete => {
                let line = match self.selected() {
                    Some(Entry::Folder(f)) => Line::from(vec![
                        Span::styled(" Delete folder ", Style::new().fg(colors::DANGER)),
                        Span::styled(f.name.clone(), Style::new().fg(colors::TEXT).bold()),
                        Span::styled(
                            " and everything inside it? <y/n>",
                            Style::new().fg(colors::DANGER),
                        ),
                    ]),
                    Some(Entry::Note(n)) => Line::from(vec![
                        Span::styled(" Delete ", Style::new().fg(colors::DANGER)),
                        Span::styled(n.title.clone(), Style::new().fg(colors::TEXT).bold()),
                        Span::styled("? <y/n>", Style::new().fg(colors::DANGER)),
                    ]),
                    None => Line::from(""),
                };
                frame.render_widget(
                    Paragraph::new(line).block(pane_block(" Confirm ")),
                    panes[1],
                );
            }
            Mode::Navigate => {
                let (title, p) = self.preview();
                frame.render_widget(
                    p.block(pane_block(&title)).wrap(Wrap { trim: false }),
                    panes[1],
                );
            }
        }

        frame.render_widget(self.instructions(), layout[2]);
    }

    fn cursor_position(&self) -> Option<Position> {
        if self.mode != Mode::Create {
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
        Some(Position::new(inner.x + self.input_cursor as u16, inner.y))
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
