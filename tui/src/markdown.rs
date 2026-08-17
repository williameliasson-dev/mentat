//! Renders markdown into ratatui `Line`s, mapped onto the mentat theme.

use minimad::{Composite, CompositeStyle, Compound, Line as MdLine, parse_line};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::consts::colors;

/// Renders a markdown document to styled ratatui lines, one per source line.
/// Line-by-line means no cross-line constructs (multi-line code fences keep
/// their fence markers) — acceptable for a preview pane.
pub fn render(md: &str) -> Vec<Line<'static>> {
    md.lines().map(|l| render_line(parse_line(l))).collect()
}

fn render_line(md: MdLine) -> Line<'static> {
    match md {
        MdLine::Normal(c) => composite_line(&c),
        MdLine::CodeFence(_) => Line::from(Span::styled("```", code_style())),
        MdLine::HorizontalRule => {
            Line::from(Span::styled("─".repeat(40), Style::new().fg(colors::DIM)))
        }
        // Tables render as raw rows for now.
        MdLine::TableRow(row) => Line::from(
            row.cells
                .iter()
                .flat_map(|c| {
                    c.compounds
                        .iter()
                        .map(compound_span)
                        .chain([Span::raw(" │ ")])
                })
                .collect::<Vec<_>>(),
        ),
        MdLine::TableRule(_) => {
            Line::from(Span::styled("─".repeat(20), Style::new().fg(colors::DIM)))
        }
    }
}

fn composite_line(c: &Composite) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let base = match &c.style {
        CompositeStyle::Header(_) => {
            // Heading text only — sand + bold signals the level well enough.
            Style::new().fg(colors::SAND).add_modifier(Modifier::BOLD)
        }
        CompositeStyle::ListItem(depth) => {
            spans.push(Span::styled(
                format!("{}• ", "  ".repeat(depth.saturating_sub(1) as usize)),
                Style::new().fg(colors::SAND),
            ));
            Style::new().fg(colors::TEXT)
        }
        CompositeStyle::OrderedListItem { level, index } => {
            spans.push(Span::styled(
                format!(
                    "{}{}. ",
                    "  ".repeat(level.saturating_sub(1) as usize),
                    index
                ),
                Style::new().fg(colors::SAND),
            ));
            Style::new().fg(colors::TEXT)
        }
        CompositeStyle::Code => {
            spans.push(Span::styled("  ", code_style()));
            code_style()
        }
        CompositeStyle::Quote => {
            spans.push(Span::styled("▌ ", Style::new().fg(colors::SAND)));
            Style::new().fg(colors::DIM).add_modifier(Modifier::ITALIC)
        }
        CompositeStyle::Paragraph => Style::new().fg(colors::TEXT),
    };

    spans.extend(c.compounds.iter().enumerate().map(|(i, cp)| {
        compound_span_base(
            cp,
            base,
            matches!(c.style, CompositeStyle::Header(_)) && i == 0,
        )
    }));
    Line::from(spans)
}

/// Inline compound inside a composite, inheriting the composite's base style.
/// For the first compound of a header, minimad keeps the `#`/`##` marker in
/// the source — strip it.
fn compound_span_base(c: &Compound, base: Style, strip_header_marker: bool) -> Span<'static> {
    let mut style = base;
    if c.code {
        style = code_style();
    }
    if c.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if c.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if c.strikeout {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    let src = if strip_header_marker {
        c.src.trim_start_matches('#').trim_start().to_string()
    } else {
        c.src.to_string()
    };
    Span::styled(src, style)
}

/// Inline compound with no composite context (table cells).
fn compound_span(c: &Compound) -> Span<'static> {
    compound_span_base(c, Style::new().fg(colors::TEXT), false)
}

fn code_style() -> Style {
    Style::new().fg(colors::DARK).bg(colors::SAND)
}
