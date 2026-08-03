use crossterm::event::Event;
use ratatui::Frame;

pub mod views;

pub trait View {
    fn handle_events(&self, event: Event);
    fn render(&self, frame: &mut Frame);
}

pub enum Action {
    None,
    Exit,
    SwitchTo(Box<dyn View>),
}
