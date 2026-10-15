use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

pub(super) const BOLD: Style = Style::new().add_modifier(Modifier::BOLD);
pub(super) const DIM: Style = Style::new().add_modifier(Modifier::DIM);
pub(super) const REVERSED: Style = Style::new().add_modifier(Modifier::REVERSED);

/// A cell's runs as spans: matched characters bold, the rest in `base`.
pub(super) fn cell<'a>(runs: &[(&'a str, bool)], base: Style) -> Line<'a> {
    runs.iter()
        .map(|&(text, matched)| Span::styled(text, if matched { BOLD } else { base }))
        .collect()
}
