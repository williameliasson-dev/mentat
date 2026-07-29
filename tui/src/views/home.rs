use crossterm::event::Event;
use ratatui::Frame;

use crate::{Action, View};

pub struct HomeView {}

impl View for HomeView {
    fn handle_events(&self, event: Event) -> Action {
        Action::None
    }

    fn render(&self, frame: &mut Frame) {
        frame.render_widget("aaa", frame.area());
    }
}
