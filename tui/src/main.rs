use crossterm::event::Event;
use ratatui::{DefaultTerminal, Frame};

trait View {
    fn handle_events(&self, event: Event) -> Action;
    fn render(&self, frame: &mut Frame);
}

struct HomeView {}

impl View for HomeView {
    fn handle_events(&self, event: Event) -> Action {
        Action::None
    }

    fn render(&self, frame: &mut Frame) {
        frame.render_widget("aaa", frame.area());
    }
}

enum Action {
    None,
    Exit,
    SwitchTo(Box<dyn View>),
}

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
