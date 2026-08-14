use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Position;

pub mod consts;
pub mod views;

pub trait View {
    fn handle_events(&mut self, event: Event) -> Option<Action>;
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
    fn set_note_body(&mut self, _id: i64, _body: &str) {}
}

pub enum Action {
    None,
    Exit,
    SwitchTo(Box<dyn View>),
    /// App-level: switch to the notes view (App owns the NoteService).
    ShowNotes,
    /// App-level: open the note in an external editor, then persist changes.
    EditNote(i64),
}
