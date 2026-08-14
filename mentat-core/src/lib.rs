pub mod error;
pub mod note;
pub mod repository;
pub mod service;

pub use error::{CoreError, Result};
pub use note::Note;
pub use repository::NoteRepository;
pub use service::NoteService;
