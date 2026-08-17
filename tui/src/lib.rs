use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Position;

use mentat_core::{NoteService, Repositories};

pub mod consts;
pub mod markdown;
pub mod views;

/// Every long-lived service, built once at startup and owned by `App`.
///
/// Views are lent this per call rather than holding it, so no view keeps a
/// service it doesn't use and none can hold a stale handle. Adding a service
/// means adding a field here and a line in `new` — no view signatures or
/// `Action` variants change.
pub struct Services {
    pub notes: NoteService,
}

impl Services {
    pub fn new(repositories: Repositories) -> Self {
        Self {
            notes: NoteService::new(repositories.notes),
        }
    }
}

pub trait View {
    fn handle_events(&mut self, services: &Services, event: Event) -> Option<Action>;
    fn render(&self, frame: &mut Frame);

    /// Where the terminal cursor should sit after rendering, if anywhere.
    fn cursor_position(&self) -> Option<Position> {
        None
    }

    /// Body of the given note, if this view exposes one (used by external editing).
    fn note_body(&self, _id: i64) -> Option<String> {
        None
    }

    /// Persist an externally-edited body for the given note.
    fn set_note_body(&mut self, _svc: &Services, _id: i64, _body: &str) {}
}

pub enum Action {
    None,
    Exit,
    SwitchTo(Box<dyn View>),
    /// App-level: open the note in an external editor, then persist changes.
    /// Kept separate because it requires terminal suspend/resume, which only
    /// App can do — views can't reach the terminal.
    EditNote(i64),
}
