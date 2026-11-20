use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, BorderType};

use super::style::{Role, Theme};

const RULE: &str = "─";
const LEFT_TEE: &str = "├";
const RIGHT_TEE: &str = "┤";

/// A rounded box in `area`. Returns the room inside it.
pub(super) fn border(frame: &mut Frame, area: Rect, theme: Theme) -> Rect {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.style(Role::Stroke));
    let inside = block.inner(area);
    frame.render_widget(block, area);
    inside
}

/// A line across `row`, which is inside the box, joined to both sides of it.
pub(super) fn rule(frame: &mut Frame, row: Rect, theme: Theme) {
    if row.is_empty() || row.x == 0 {
        return;
    }
    let style = theme.style(Role::Stroke);
    let buffer = frame.buffer_mut();
    buffer.set_string(row.x - 1, row.y, LEFT_TEE, style);
    buffer.set_string(row.x, row.y, RULE.repeat(usize::from(row.width)), style);
    buffer.set_string(row.right(), row.y, RIGHT_TEE, style);
}
