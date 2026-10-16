use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Position;
use uuid::Uuid;

pub mod consts;
pub mod markdown;
pub mod views;

use daemon::Services;

pub trait View {
    fn handle_events(&mut self, services: &Services, event: Event) -> Option<Action>;
    fn render(&self, frame: &mut Frame);

    /// Where the terminal cursor should sit after rendering, if anywhere.
    fn cursor_position(&self) -> Option<Position> {
        None
    }

    /// Body of the given note, if this view exposes one (used by external editing).
    fn note_body(&self, _id: Uuid) -> Option<String> {
        None
    }

    /// Persist an externally-edited body for the given note.
    fn set_note_body(&mut self, _svc: &Services, _id: Uuid, _body: &str) {}
}

pub enum Action {
    None,
    Exit,
    SwitchTo(Box<dyn View>),
    /// App-level: open the note in an external editor, then persist changes.
    /// Kept separate because it requires terminal suspend/resume, which only
    /// App can do — views can't reach the terminal.
    EditNote(Uuid),
}
