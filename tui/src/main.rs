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
            match self.view.handle_events(event) {
                Action::None => (),
                Action::SwitchTo(view) => self.view = view,
                Action::Exit => (),
            }
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
}

fn main() -> color_eyre::Result<()> {
    let mut app: App = App::new();

    color_eyre::install()?;
    app.run();

    Ok(())
}
