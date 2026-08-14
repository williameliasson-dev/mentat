//! Launch an external editor ($VISOR, $EDITOR, or nvim) on a note's body.
//!
//! The TUI is suspended while the editor runs. If the editor exits
//! successfully and the file changed, the new content is returned.

use std::io::Write;
use std::process::Command;

/// Opens `body` in the user's editor inside the already-restored terminal.
/// Returns the edited content, or `None` if the editor failed or the file
/// was left unchanged.
///
/// The caller is responsible for suspending/restoring the TUI around this.
pub fn edit_body(body: &str) -> std::io::Result<Option<String>> {
    let mut path = std::env::temp_dir();
    path.push(format!("mentat-{}.md", std::process::id()));

    std::fs::File::create(&path)?.write_all(body.as_bytes())?;

    let status = Command::new(editor_program()).arg(&path).status();

    let result = match status {
        Ok(s) if s.success() => {
            let edited = std::fs::read_to_string(&path)?;
            (edited != body).then_some(edited)
        }
        _ => None,
    };

    let _ = std::fs::remove_file(&path);
    Ok(result)
}

fn editor_program() -> String {
    std::env::var("VISOR")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "nvim".into())
}
