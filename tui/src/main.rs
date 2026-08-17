use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use tui::Database;
use tui::{Action, Services, View, views::HomeView};

mod editor;

struct App {
    services: Services,
    view: Box<dyn View>,
}

impl App {
    /// Opens the database and builds the initial view. Runs before the TUI is
    /// initialised, so a failure reports to a normal terminal.
    pub fn new() -> color_eyre::Result<Self> {
        let database = Database::new()?;
        Ok(App {
            services: Services::new(database.repositories()),
            view: Box::new(HomeView::new()),
        })
    }

    fn run_loop(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        loop {
            terminal.draw(|frame| {
                self.view.render(frame);
                if let Some(pos) = self.view.cursor_position() {
                    frame.set_cursor_position(pos);
                }
            })?;

            let event = crossterm::event::read()?;

            let action = self
                .view
                .handle_events(&self.services, event.clone())
                .or_else(|| self.handle_global(event));

            match action {
                Some(Action::Exit) => break Ok(()),
                Some(Action::SwitchTo(view)) => self.view = view,
                Some(Action::EditNote(id)) => self.edit_in_external_editor(terminal, id)?,
                Some(Action::None) | None => {}
            }
        }
    }

    /// Suspends the TUI, opens the note in $VISOR/$EDITOR/nvim, saves changes.
    fn edit_in_external_editor(
        &mut self,
        terminal: &mut DefaultTerminal,
        id: i64,
    ) -> color_eyre::Result<()> {
        let Some(body) = self.view.note_body(id) else {
            return Ok(());
        };

        ratatui::restore();
        let edited = editor::edit_body(&body);
        *terminal = ratatui::init();

        if let Some(new_body) = edited? {
            self.view.set_note_body(&self.services, id, &new_body);
        }
        Ok(())
    }

    pub fn run(&mut self) -> color_eyre::Result<()> {
        ratatui::run(|terminal| self.run_loop(terminal))
    }

    fn handle_global(&self, event: Event) -> Option<Action> {
        if let Event::Key(key) = event
            && key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return Some(Action::Exit);
        }
        None
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    App::new()?.run()
}
