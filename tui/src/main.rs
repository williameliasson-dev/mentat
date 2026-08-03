use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use tui::{Action, View, views::HomeView};

struct App {
    view: Box<dyn View>,
}

impl App {
    fn run_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(|frame| self.view.render(frame))?;
            let event = crossterm::event::read()?;

            if let Some(action) = self.handle_global(event.clone()) {
                match action {
                    Action::Exit => break Ok(()),
                    Action::SwitchTo(view) => self.view = view,
                    _ => {}
                }
            }

            self.view.handle_events(event);
        }
    }

    pub fn new() -> Self {
        App {
            view: Box::new(HomeView {}),
        }
    }

    pub fn run(&mut self) -> Result<(), std::io::Error> {
        ratatui::run(|terminal| self.run_loop(terminal))
    }
    fn handle_global(&self, event: Event) -> Option<Action> {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(Action::Exit)
                }
                _ => None,
            }
        } else {
            None
        }
    }
}

fn main() -> color_eyre::Result<()> {
    let mut app: App = App::new();

    color_eyre::install()?;
    app.run();

    Ok(())
}
