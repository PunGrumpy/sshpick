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
const ARROW: &str = "→";
const RESOLVING: &str = "→ resolving…";
const VIA: &str = "via";
const RUNS: &str = "runs";
const PLAIN: Style = Style::new();

pub(super) fn route(frame: &mut Frame, area: Rect, slot: Option<&Slot>) {
    let Some(slot) = slot else {
        return;
    };
    let (target, headline);
    let mut parts = vec![(INDENT, PLAIN)];
    match slot {
        Slot::Waiting => parts.push((RESOLVING, DIM)),
        Slot::Failed(error) => {
            headline = error.headline();
            parts.extend([(ARROW, DIM), (" ", PLAIN), (&headline, PLAIN)]);
        }
        Slot::Ready(preview) => {
            let Some(route) = preview.route() else {
                return;
            };
            target = route.target();
            parts.extend([(ARROW, DIM), (" ", PLAIN), (&target, PLAIN)]);
            for (joiner, word) in [(VIA, route.via), (RUNS, route.runs)] {
                if let Some(word) = word {
                    parts.extend([(" ", PLAIN), (joiner, DIM), (" ", PLAIN), (word, PLAIN)]);
                }
            }
        }
    }
    let spans = fit_parts(&parts, usize::from(area.width), PLAIN)
        .into_iter()
        .map(|(text, style)| Span::styled(text, style));
    frame.render_widget(Line::from_iter(spans), area);
}
