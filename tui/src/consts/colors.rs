//! Central color theme — the single place to tune mentat's look.
//!
//! Palette is Dune-inspired: sands of Arrakis with spice accents.

use ratatui::style::Color;

/// Primary: Arrakis sand. Titles, borders, key hints, accents.
pub const SAND: Color = Color::Rgb(194, 154, 91);

/// Muted: deep desert shadow. Secondary text, hints, quotes.
pub const DIM: Color = Color::Rgb(120, 100, 70);

/// Regular body text.
pub const TEXT: Color = Color::Rgb(210, 200, 185);

/// Ibad blue: the spice-stained eyes, dusted with sand. Note accents — reads
/// as a distinct hue from the folders without breaking the warm palette, so
/// it's desaturated and matched to SAND's luminance rather than a true blue.
pub const IBAD: Color = Color::Rgb(127, 153, 165);

/// Danger: destructive confirmations (delete prompts).
pub const DANGER: Color = Color::Rgb(200, 90, 70);

/// Dark: foreground for text sitting on a SAND background (code chips).
pub const DARK: Color = Color::Rgb(30, 25, 20);
