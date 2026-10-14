use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

pub(super) const MATCHED: Style = Style::new().add_modifier(Modifier::BOLD);
pub(super) const DIM: Style = Style::new().add_modifier(Modifier::DIM);

pub(super) fn cell<'a>(runs: &[(&'a str, bool)], base: Style) -> Line<'a> {
    runs.iter()
        .map(|&(text, matched)| Span::styled(text, if matched { MATCHED } else { base }))
        .collect()
}
