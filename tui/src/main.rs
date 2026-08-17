use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use mentat_core::{NoteRepository, NoteService};
use ratatui::DefaultTerminal;
use tui::{Action, Services, View, views::HomeView};

mod editor;

struct App {
    services: Services,
    view: Box<dyn View>,
}

impl App {
    pub fn new() -> color_eyre::Result<Self> {
        Ok(App {
            services: Services {
                notes: NoteService::new(NoteRepository::open(db_path())?),
            },
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

/// Platform-idiomatic database location:
/// Linux: `~/.local/share/mentat/mentat.db`
/// macOS: `~/Library/Application Support/dev.mentat/mentat.db`
fn db_path() -> PathBuf {
    let dirs = directories::ProjectDirs::from("dev", "", "mentat")
        .expect("could not determine data directory");
    let dir = dirs.data_dir();
    std::fs::create_dir_all(dir).expect("failed to create data directory");
    dir.join("mentat.db")
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    App::new()?.run()
}
