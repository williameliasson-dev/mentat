use ratatui::DefaultTerminal;

enum ViewName {
    Home,
    Notes,
}

struct App {
    view: ViewName,
}

impl App {
    fn run_loop(&self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        self.render_view(terminal)
    }

    fn render_view(&self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        match self.view {
            ViewName::Home => loop {
                terminal.draw(|frame| frame.render_widget("home", frame.area()))?;
                if crossterm::event::read()?.is_key_press() {
                    break Ok(());
                }
            },

            ViewName::Notes => loop {
                terminal.draw(|frame| frame.render_widget("notes", frame.area()))?;
                if crossterm::event::read()?.is_key_press() {
                    break Ok(());
                }
            },
        }
    }

    pub fn new() -> Self {
        App {
            view: ViewName::Home,
        }
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
