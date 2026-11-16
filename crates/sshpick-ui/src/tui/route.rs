//! Values come from ssh and may hold control characters: they reach the buffer through `Span`,
//! which drops them.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use sshpick_core::Slot;

use super::style::DIM;
use crate::text::fit_parts;

const INDENT: &str = "  ";
const CORNER: &str = "└";
const RESOLVING: &str = "resolving…";
const VIA: &str = "via";
const RUNS: &str = "runs";
const PLAIN: Style = Style::new();

/// Whether `line` has anything to say. The list leaves a row for the line only when it does, so a
/// host with no `hostname` has no blank row under it.
pub(super) fn has_line(slot: &Slot) -> bool {
    match slot {
        Slot::Ready(preview) => preview.route().is_some(),
        Slot::Waiting | Slot::Failed(_) => true,
    }
}

/// One row under the selected host: where it goes, `resolving…`, or why `ssh -G` failed.
pub(super) fn line(frame: &mut Frame, area: Rect, slot: &Slot) {
    let (target, headline);
    let mut parts = vec![(INDENT, PLAIN)];
    match slot {
        Slot::Waiting => parts.extend([(CORNER, DIM), (" ", DIM), (RESOLVING, DIM)]),
        Slot::Failed(error) => {
            headline = error.headline();
            parts.extend([(CORNER, DIM), (" ", PLAIN), (&headline, PLAIN)]);
        }
        Slot::Ready(preview) => {
            let Some(route) = preview.route() else {
                return;
            };
            target = route.target();
            parts.extend([(CORNER, DIM), (" ", PLAIN), (&target, PLAIN)]);
            for (joiner, word) in [(VIA, route.via), (RUNS, route.runs)] {
                if let Some(word) = word {
                    parts.extend([(" ", PLAIN), (joiner, DIM), (" ", PLAIN), (word, PLAIN)]);
                }
            }
        }
    }
    frame.render_widget(
        spans(&fit_parts(&parts, usize::from(area.width), PLAIN)),
        area,
    );
}

fn spans<'a>(parts: &[(&'a str, Style)]) -> Line<'a> {
    parts
        .iter()
        .map(|&(text, style)| Span::styled(text, style))
        .collect()
}
