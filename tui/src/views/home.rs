use crossterm::event::{Event, KeyCode};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    Action, Services, View,
    consts::colors::{DIM, SAND},
};

const BANNER: &[&str] = &[
    r"███╗   ███╗███████╗███╗   ██╗████████╗ █████╗ ████████╗",
    r"████╗ ████║██╔════╝████╗  ██║╚══██╔══╝██╔══██╗╚══██╔══╝",
    r"██╔████╔██║█████╗  ██╔██╗ ██║   ██║   ███████║   ██║   ",
    r"██║╚██╔╝██║██╔══╝  ██║╚██╗██║   ██║   ██╔══██║   ██║   ",
    r"██║ ╚═╝ ██║███████╗██║ ╚████║   ██║   ██║  ██║   ██║   ",
    r"╚═╝     ╚═╝╚══════╝╚═╝  ╚═══╝   ╚═╝   ╚═╝  ╚═╝   ╚═╝   ",
];

/// Braille-rendered mentat, baked in at compile time.
const MENTAT_ART: &str = include_str!("mentat.txt");

const QUOTE: &str = "\"It is by will alone I set my mind in motion.\"";

const MENU: &[(&str, &str)] = &[("n", "Notes"), ("q", "Quit")];

/// Braille chars the converter used for fully-blank cells.
const BLANK: [char; 3] = ['\u{2800}', '\u{2804}', ' '];

/// Bounding box of the non-blank figure inside the padded art file.
struct ArtBounds {
    top: usize,
    left: usize,
    width: usize,
    height: usize,
}

fn art_bounds(art: &str) -> ArtBounds {
    let lines: Vec<&str> = art.lines().collect();
    fn has_content(l: &str) -> bool {
        l.chars().any(|c| !BLANK.contains(&c))
    }

    let top = lines.iter().position(|l| has_content(l)).unwrap_or(0);
    let height = lines
        .iter()
        .rposition(|l| has_content(l))
        .map(|b| b - top + 1)
        .unwrap_or(0);

    let left = lines
        .iter()
        .filter(|l| has_content(l))
        .map(|l| l.chars().take_while(|c| BLANK.contains(c)).count())
        .min()
        .unwrap_or(0);
    let right = lines
        .iter()
        .filter(|l| has_content(l))
        .map(|l| {
            let trailing = l.chars().rev().take_while(|c| BLANK.contains(c)).count();
            l.chars().count() - trailing
        })
        .max()
        .unwrap_or(0);

    ArtBounds {
        top,
        left,
        width: right.saturating_sub(left),
        height,
    }
}

/// The cropped figure, line by line.
fn art_lines(art: &str, b: &ArtBounds) -> Vec<Line<'static>> {
    art.lines()
        .skip(b.top)
        .take(b.height)
        .map(|l| {
            let cropped: String = l.chars().skip(b.left).take(b.width).collect();
            Line::from(Span::styled(cropped, Style::new().fg(SAND)))
        })
        .collect()
}

#[derive(Default)]
pub struct HomeView {
    selected: usize,
}

impl HomeView {
    pub fn new() -> Self {
        Self { selected: 0 }
    }

    fn menu_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
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
        lines
    }

    fn footer_lines() -> Vec<Line<'static>> {
        vec![
            Line::from(Span::styled(
                QUOTE,
                Style::new().fg(DIM).add_modifier(Modifier::ITALIC),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(" select ", Style::new().fg(DIM)),
                Span::styled("<j/k>", Style::new().fg(SAND).bold()),
                Span::styled("  open ", Style::new().fg(DIM)),
                Span::styled("<enter>", Style::new().fg(SAND).bold()),
            ]),
        ]
    }
}

impl View for HomeView {
    fn handle_events(&mut self, services: &Services, event: Event) -> Option<Action> {
        let Event::Key(key) = event else { return None };
        match key.code {
            KeyCode::Char('q') => Some(Action::Exit),
            KeyCode::Char('n') => Some(Action::SwitchTo(Box::new(crate::views::NotesView::new(
                services,
            )))),
            KeyCode::Char('j') | KeyCode::Down => {
                self.selected = (self.selected + 1).min(MENU.len() - 1);
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            KeyCode::Enter | KeyCode::Char(' ') => match MENU[self.selected].0 {
                "n" => Some(Action::SwitchTo(Box::new(crate::views::NotesView::new(
                    services,
                )))),
                _ => Some(Action::Exit),
            },
            _ => None,
        }
    }

    fn render(&self, frame: &mut Frame) {
        let bounds = art_bounds(MENTAT_ART);

        let menu = self.menu_lines();
        let footer = Self::footer_lines();

        // Full layout: figure + menu + quote. Fallback (small terminal):
        // banner + menu only.
        let full_height = bounds.height + 1 + menu.len() + 1 + footer.len();
        let use_full = frame.area().width as usize >= bounds.width + 4
            && frame.area().height as usize >= full_height + 2;

        let banner: Vec<Line> = BANNER
            .iter()
            .map(|l| Line::from(Span::styled(*l, Style::new().fg(SAND))))
            .collect();

        let lines: Vec<Line> = if use_full {
            let mut lines = banner.clone();
            lines.push(Line::from(""));
            lines.extend(art_lines(MENTAT_ART, &bounds));
            lines.push(Line::from(""));
            lines.extend(menu);
            lines.push(Line::from(""));
            lines.extend(footer);
            lines
        } else {
            let mut lines = banner.clone();
            lines.push(Line::from(""));
            lines.extend(menu);
            lines
        };

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
