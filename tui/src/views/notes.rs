use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{Action, Services, View, consts::colors};
use mentat_core::Note;

const TITLE_MAX: usize = 60;

#[derive(PartialEq)]
enum Mode {
    Navigate,
    NewTitle,
    ConfirmDelete,
}

pub struct NotesView {
    notes: Vec<Note>,
    list_state: ListState,
    mode: Mode,
    title_input: String,
    /// Byte cursor within `title_input`.
    input_cursor: usize,
}

impl NotesView {
    pub fn new(svc: &Services) -> Self {
        let mut view = Self {
            notes: Vec::new(),
            list_state: ListState::default(),
            mode: Mode::Navigate,
            title_input: String::new(),
            input_cursor: 0,
        };
        view.reload(svc);
        view
    }

    fn reload(&mut self, svc: &Services) {
        self.notes = svc.notes.list_notes().unwrap_or_default();
        if self.notes.is_empty() {
            self.list_state.select(None);
        } else {
            let i = self
                .list_state
                .selected()
                .unwrap_or(0)
                .min(self.notes.len() - 1);
            self.list_state.select(Some(i));
        }
    }

    fn select_next(&mut self) {
        if self.notes.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((i + 1).min(self.notes.len() - 1)));
    }

    fn select_previous(&mut self) {
        if self.notes.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(i.saturating_sub(1)));
    }

    fn selected_note(&self) -> Option<&Note> {
        self.list_state.selected().and_then(|i| self.notes.get(i))
    }

    fn selected_id(&self) -> Option<i64> {
        self.selected_note().map(|n| n.id)
    }

    fn instructions(&self) -> Line<'static> {
        let key = |s: &'static str| Span::styled(s, Style::new().fg(colors::SAND).bold());
        let text = |s: &'static str| Span::styled(s, Style::new().fg(colors::DIM));
        match self.mode {
            Mode::Navigate => Line::from(vec![
                text(" New "),
                key("<n>"),
                text(" Edit "),
                key("<enter>"),
                text(" Delete "),
                key("<d>"),
                text(" Home "),
                key("<h>"),
                text(" Quit "),
                key("<q>"),
            ]),
            Mode::NewTitle => Line::from(vec![
                text(" Create "),
                key("<Enter>"),
                text(" Cancel "),
                key("<Esc>"),
            ]),
            Mode::ConfirmDelete => Line::from(vec![
                text(" Delete note? "),
                Span::styled("<y>", Style::new().fg(colors::DANGER).bold()),
                text(" / "),
                key("<n>"),
            ]),
        }
    }

    fn handle_navigate(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('q') => Some(Action::Exit),
            KeyCode::Char('h') => Some(Action::SwitchTo(Box::new(crate::views::HomeView::new()))),
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next();
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_previous();
                None
            }
            KeyCode::Char('n') => {
                self.mode = Mode::NewTitle;
                self.title_input.clear();
                self.input_cursor = 0;
                None
            }
            KeyCode::Enter => self.selected_id().map(Action::EditNote),
            KeyCode::Char('d') => {
                if self.selected_note().is_some() {
                    self.mode = Mode::ConfirmDelete;
                }
                None
            }
            _ => None,
        }
    }

    fn handle_new_title(&mut self, svc: &Services, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Navigate;
            }
            KeyCode::Enter => {
                let title = self.title_input.trim();
                if !title.is_empty() {
                    let _ = svc.notes.create_note(title, "");
                    self.reload(svc);
                    self.list_state.select(Some(0));
                    self.mode = Mode::Navigate;
                }
            }
            _ => self.edit_title_input(key),
        }
        None
    }

    fn handle_confirm_delete(&mut self, svc: &Services, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('y') => {
                if let Some(id) = self.selected_id() {
                    let _ = svc.notes.delete_note(id);
                    self.reload(svc);
                }
                self.mode = Mode::Navigate;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = Mode::Navigate;
            }
            _ => {}
        }
        None
    }

    /// Single-line input editing at `input_cursor` over `title_input`.
    fn edit_title_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                if self.title_input.len() < TITLE_MAX {
                    self.title_input.insert(self.input_cursor, c);
                    self.input_cursor += c.len_utf8();
                }
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    let prev = prev_char_boundary(&self.title_input, self.input_cursor);
                    self.title_input.replace_range(prev..self.input_cursor, "");
                    self.input_cursor = prev;
                }
            }
            KeyCode::Delete => {
                if self.input_cursor < self.title_input.len() {
                    let next = next_char_boundary(&self.title_input, self.input_cursor);
                    self.title_input.replace_range(self.input_cursor..next, "");
                }
            }
            KeyCode::Left => {
                self.input_cursor = prev_char_boundary(&self.title_input, self.input_cursor)
            }
            KeyCode::Right => {
                self.input_cursor = next_char_boundary(&self.title_input, self.input_cursor)
            }
            KeyCode::Home => self.input_cursor = 0,
            KeyCode::End => self.input_cursor = self.title_input.len(),
            _ => {}
        }
    }
}

impl View for NotesView {
    fn handle_events(&mut self, services: &Services, event: Event) -> Option<Action> {
        let Event::Key(key) = event else { return None };
        match self.mode {
            Mode::Navigate => self.handle_navigate(key),
            Mode::NewTitle => self.handle_new_title(services, key),
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

        // Note list
        let items: Vec<ListItem> = self
            .notes
            .iter()
            .map(|n| ListItem::new(n.title.as_str()).style(Style::new().fg(colors::TEXT)))
            .collect();
        let list = List::new(items)
            .block(pane_block(" Notes "))
            .highlight_style(Style::new().fg(Color::Black).bg(colors::SAND).bold())
            .highlight_symbol("> ");
        let mut state = self.list_state;
        frame.render_stateful_widget(list, panes[0], &mut state);

        // Right pane depends on mode
        match self.mode {
            Mode::NewTitle => {
                let p = Paragraph::new(self.title_input.as_str())
                    .style(Style::new().fg(colors::TEXT))
                    .block(pane_block(" New note title "));
                frame.render_widget(p, panes[1]);
            }
            Mode::ConfirmDelete => {
                let title = self.selected_note().map(|n| n.title.as_str()).unwrap_or("");
                let p = Paragraph::new(Line::from(vec![
                    Span::styled(" Delete ", Style::new().fg(colors::DANGER)),
                    Span::styled(title, Style::new().fg(colors::TEXT).bold()),
                    Span::styled("? <y/n>", Style::new().fg(colors::DANGER)),
                ]))
                .block(pane_block(" Confirm "));
                frame.render_widget(p, panes[1]);
            }
            Mode::Navigate => {
                let title = self
                    .selected_note()
                    .map(|n| format!(" {} ", n.title))
                    .unwrap_or_else(|| " Preview ".into());
                let p = match self.selected_note() {
                    Some(n) => Paragraph::new(crate::markdown::render(&n.body)),
                    None if self.notes.is_empty() => {
                        Paragraph::new("No notes yet. Press <n> to create one.")
                            .style(Style::new().fg(colors::TEXT))
                    }
                    None => Paragraph::new(""),
                };
                frame.render_widget(
                    p.block(pane_block(&title)).wrap(Wrap { trim: false }),
                    panes[1],
                );
            }
        }

        frame.render_widget(self.instructions(), layout[2]);
    }

    fn cursor_position(&self) -> Option<Position> {
        if self.mode != Mode::NewTitle {
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
        self.notes
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.body.clone())
    }

    fn set_note_body(&mut self, svc: &Services, id: i64, body: &str) {
        let Some(note) = self.notes.iter().find(|n| n.id == id) else {
            return;
        };
        let title = note.title.clone();
        let _ = svc.notes.update_note(id, &title, body);
        self.reload(svc);
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
