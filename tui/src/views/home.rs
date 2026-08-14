use crossterm::event::Event;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use crate::View;

pub struct HomeView {}

impl View for HomeView {
    fn handle_events(&self, event: Event) {}

    fn render(&self, frame: &mut Frame) {
        let text = vec![
            "".into(),
            "Daily Note".into(),
            "tutorial-swag.md".into(),
            "Third line".into(),
        ];

        let instructions = Line::from(vec![
            " Binds ".into(),
            "<?>".blue().bold(),
            " Quit ".into(),
            "<Ctrl-C> ".blue().bold(),
        ]);

        let block = Block::bordered().border_set(border::THICK);

        let paragraph = Paragraph::new(text)
            .block(block)
            .style(Style::new().white().on_black());

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(4),
                Constraint::Percentage(95),
                Constraint::Percentage(1),
            ])
            .split(frame.area());

        frame.render_widget(
            Paragraph::new("Tabs").block(Block::new().borders(Borders::ALL)),
            layout[0],
        );

        frame.render_widget(paragraph, layout[1]);
        frame.render_widget(instructions, layout[2]);
    }
}
