use ratatui::{DefaultTerminal, Frame};

enum View {
    Home,
    Notes,
}

struct App {
    view: View,
}

impl App {
    fn run_loop(&self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(|frame| self.render_view(frame, &self.view))?;
            if crossterm::event::read()?.is_key_press() {
                break Ok(());
            }
        }
    }

    fn render_view(&self, frame: &mut Frame, view: &View) {
        match view {
            View::Home => frame.render_widget("home", frame.area()),
            View::Notes => frame.render_widget("notes", frame.area()),
        }
    }

    pub fn new() -> Self {
        App { view: View::Home }
    }

    pub fn run(&self) -> Result<(), std::io::Error> {
        ratatui::run(|terminal| self.run_loop(terminal))
    }
}

fn main() -> color_eyre::Result<()> {
    let app: App = App::new();

    color_eyre::install()?;
    app.run();

    Ok(())
}
