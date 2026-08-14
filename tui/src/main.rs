use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use tui::{Action, View, views::NotesView};

mod editor;

struct App {
    view: Box<dyn View>,
    db_path: PathBuf,
}

impl App {
    pub fn new() -> Self {
        let db_path = PathBuf::from("mentat.db");
        let view = NotesView::open(&db_path)
            .map(|v| Box::new(v) as Box<dyn View>)
            .expect("failed to open notes database");
        App { view, db_path }
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
                .handle_events(event.clone())
                .or_else(|| self.handle_global(event));

            match action {
                Some(Action::Exit) => break Ok(()),
                Some(Action::SwitchTo(view)) => self.view = view,
                Some(Action::ShowNotes) => {
                    self.view = Box::new(NotesView::open(&self.db_path)?);
                }
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
            self.view.set_note_body(id, &new_body);
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
    App::new().run()
}
