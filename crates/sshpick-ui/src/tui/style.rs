use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use sshpick_core::Run;

pub(super) const BOLD: Style = Style::new().add_modifier(Modifier::BOLD);
pub(super) const DIM: Style = Style::new().add_modifier(Modifier::DIM);
pub(super) const REVERSED: Style = Style::new().add_modifier(Modifier::REVERSED);

pub(super) fn cell<'a>(runs: &[Run<'a>], base: Style) -> Line<'a> {
    runs.iter()
        .map(|run| Span::styled(run.text, if run.matched { BOLD } else { base }))
        .collect()
}
