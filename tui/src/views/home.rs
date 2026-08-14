use crossterm::event::{Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{Action, View};

const BANNER: &[&str] = &[
    r"███╗   ███╗███████╗███╗   ██╗████████╗ █████╗ ████████╗",
    r"████╗ ████║██╔════╝████╗  ██║╚══██╔══╝██╔══██╗╚══██╔══╝",
    r"██╔████╔██║█████╗  ██╔██╗ ██║   ██║   ███████║   ██║   ",
    r"██║╚██╔╝██║██╔══╝  ██║╚██╗██║   ██║   ██╔══██║   ██║   ",
    r"██║ ╚═╝ ██║███████╗██║ ╚████║   ██║   ██║  ██║   ██║   ",
    r"╚═╝     ╚═╝╚══════╝╚═╝  ╚═══╝   ╚═╝   ╚═╝  ╚═╝   ╚═╝   ",
];

/// The maw of Shai-Hulud.
const SIGNET: &[&str] = &[
    r"_.--~~~~--._",
    r".-~            ~-.",
    r"/   .-~~~~~~~~-.   \",
    r"|   /   .-~~~~-.  \   |",
    r"|  |   /  .--.  \  |  |",
    r" \  \ |  (    ) | /  /",
    r"  \  \ \  '--' / /  /",
    r"   \  \ '~~~~'  /  /",
    r"    \  '.____.'  /",
    r"     '-........-'",
];

const QUOTE: &str = "\"It is by will alone I set my mind in motion.\"";

const MENU: &[(&str, &str)] = &[("n", "Notes"), ("q", "Quit")];

/// Arrakis sand.
const SAND: Color = Color::Rgb(194, 154, 91);
/// Deep desert shadow.
const DIM: Color = Color::Rgb(120, 100, 70);

#[derive(Default)]
pub struct HomeView {
    selected: usize,
}

impl HomeView {
    pub fn new() -> Self {
        Self { selected: 0 }
    }
}

impl View for HomeView {
    fn handle_events(&mut self, event: Event) -> Option<Action> {
        let Event::Key(key) = event else { return None };
        match key.code {
            KeyCode::Char('q') => Some(Action::Exit),
            KeyCode::Char('n') => Some(Action::ShowNotes),
            KeyCode::Char('j') | KeyCode::Down => {
                self.selected = (self.selected + 1).min(MENU.len() - 1);
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            KeyCode::Enter | KeyCode::Char(' ') => match MENU[self.selected].0 {
                "n" => Some(Action::ShowNotes),
                _ => Some(Action::Exit),
            },
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame) {
        let mut lines: Vec<Line> = Vec::new();

        for l in BANNER {
            lines.push(Line::from(Span::styled(*l, Style::new().fg(SAND))));
        }
        lines.push(Line::from(""));

        for l in SIGNET {
            lines.push(Line::from(Span::styled(*l, Style::new().fg(DIM))));
        }
        lines.push(Line::from(""));

        for (i, (_key, label)) in MENU.iter().enumerate() {
            let style = if i == self.selected {
                Style::new()
                    .fg(Color::Black)
                    .bg(SAND)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(SAND)
            };
            lines.push(Line::from(Span::styled(format!("  {label:<12}"), style)));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            QUOTE,
            Style::new().fg(DIM).add_modifier(Modifier::ITALIC),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" select ", Style::new().fg(DIM)),
            Span::styled("<j/k>", Style::new().fg(SAND).bold()),
            Span::styled("  open ", Style::new().fg(DIM)),
            Span::styled("<enter>", Style::new().fg(SAND).bold()),
        ]));

        let height = lines.len() as u16;
        let width = lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16;

        let paragraph = Paragraph::new(lines).alignment(Alignment::Center);

        let [area] = Layout::vertical([Constraint::Length(height)])
            .flex(Flex::Center)
            .areas(frame.area());
        let [area] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(area);

        frame.render_widget(paragraph, area);
    }
}
