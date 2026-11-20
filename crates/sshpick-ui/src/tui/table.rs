use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use sshpick_core::{Entry, HINT_GAP, Hint, Row, Run, Slot, View, counter, hint, spread};

use super::route;
use super::style::{Role, Theme};
use crate::text::{ColumnWidths, columns, cut, fit, width};

/// Blank columns at each side of the list, and of every other row but the help.
pub(super) const MARGIN: u16 = 1;
/// Blank columns inside the bar, before its first and after its last character.
pub(super) const PADDING: u16 = 1;
const GAP: u16 = 2;
/// `now`, `59m`, `23h`, `13d`, `52w` and `99y` all fit in 4 columns.
const LAST_WIDTH: u16 = 4;
/// WHAT is left out entirely when it would keep fewer columns than this — or fewer than the
/// widest WHAT when that is shorter.
const WHAT_MIN: u16 = 8;

/// The same as `integrations/rex/sshpick.lua`.
pub(super) const PLACEHOLDER: &str = "Search host, ENV or what";

#[derive(Debug)]
pub(super) struct HostTable {
    env: u16,
    host: u16,
    what: u16,
}

/// Where each column is, as columns from the left edge of the list. `inner` is what the bar holds
/// between its paddings. A column that is left out has no place.
struct Shape {
    bar: Cols,
    host: Cols,
    what: Option<Cols>,
    env: Option<Cols>,
    last: Option<Cols>,
}

/// `len` columns from column `from`.
#[derive(Clone, Copy)]
struct Cols {
    from: u16,
    len: u16,
}

impl HostTable {
    pub(super) fn new(entries: &[Entry]) -> HostTable {
        let widths = ColumnWidths::of(entries);
        let cells = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        HostTable {
            env: widths.env.map_or(0, cells),
            host: cells(widths.host),
            what: cells(widths.what),
        }
    }

    /// One row per host: HOST at the left, ENV at the right, WHAT and LAST between them.
    pub(super) fn list(&self, frame: &mut Frame, area: Rect, view: &View<'_>, theme: Theme) {
        if view.matched == 0 {
            if view.total > 0 {
                let text = format!("No host matches \"{}\"", view.query);
                let at = place(area, area.y, 1, Cols::inside(area.width));
                frame.render_widget(
                    span_line(theme, cut(&text, usize::from(at.width)), Role::Muted),
                    at,
                );
            }
            return;
        }
        let shape = self.shape(area.width, view.recents);
        for (at, row) in view.rows.iter().enumerate() {
            let y = area.y.saturating_add(u16::try_from(at).unwrap_or(u16::MAX));
            if y >= area.bottom() {
                break;
            }
            draw(frame, area, y, row, &shape, theme);
        }
    }

    /// ENV is at the far end, whole. Before it is LAST, before that WHAT, and HOST is what is
    /// left. WHAT is cut first when the row is narrow; LAST goes when WHAT would keep fewer than
    /// `WHAT_MIN` columns — or fewer than the widest WHAT when that is shorter — WHAT goes at
    /// fewer still, and then HOST is cut.
    fn shape(&self, total: u16, recents: bool) -> Shape {
        let bar = Cols::of(total);
        let inner = Cols::inside(total);
        let env = self.env.min(inner.len);
        let beside_env = if env > 0 { env + GAP } else { 0 };
        let at_far = |len: u16, behind: u16| Cols {
            from: inner.from + inner.len - behind,
            len,
        };
        let last = recents
            && self.host + GAP + self.what.min(WHAT_MIN) + GAP + LAST_WIDTH + beside_env
                <= inner.len;
        let right = inner.from + inner.len - beside_env - if last { LAST_WIDTH + GAP } else { 0 };
        let from = inner.from + self.host + GAP;
        let what = right.saturating_sub(from);
        Shape {
            bar,
            host: Cols {
                from: inner.from,
                len: self.host.min(inner.len.saturating_sub(beside_env)),
            },
            what: (what >= WHAT_MIN.min(self.what)).then_some(Cols { from, len: what }),
            env: (env > 0).then(|| at_far(env, env)),
            last: last.then(|| at_far(LAST_WIDTH, beside_env + LAST_WIDTH)),
        }
    }
}

impl Cols {
    fn of(total: u16) -> Cols {
        Cols {
            from: MARGIN,
            len: total.saturating_sub(2 * MARGIN),
        }
    }

    /// What is left of `total` columns inside the margin and the padding.
    fn inside(total: u16) -> Cols {
        let bar = Cols::of(total);
        Cols {
            from: bar.from + PADDING,
            len: bar.len.saturating_sub(2 * PADDING),
        }
    }
}

fn place(area: Rect, y: u16, lines: u16, columns: Cols) -> Rect {
    let x = area.x.saturating_add(columns.from);
    Rect::new(x, y, columns.len, lines).intersection(area)
}

fn span_line(theme: Theme, text: String, role: Role) -> Line<'static> {
    Line::from(theme.span(text, role))
}

/// HOST, WHAT, LAST and ENV on one line. The selected row is the bar across them all.
fn draw(frame: &mut Frame, area: Rect, y: u16, row: &Row<'_>, shape: &Shape, theme: Theme) {
    let (text, context) = if row.selected {
        let bar = place(area, y, 1, shape.bar);
        frame.buffer_mut().set_style(bar, theme.style(Role::Bar));
        (Role::Bar, Role::Bar)
    } else {
        (Role::Fg, Role::Muted)
    };
    let at = |columns| place(area, y, 1, columns);
    frame.render_widget(cells(theme, &row.host, shape.host, text), at(shape.host));
    if let Some(columns) = shape.what {
        frame.render_widget(cells(theme, &row.what, columns, context), at(columns));
    }
    if let Some(columns) = shape.last {
        let last = theme.span(row.last, context);
        frame.render_widget(Line::from(last).right_aligned(), at(columns));
    }
    if let Some(columns) = shape.env {
        let env = cells(theme, &row.env, columns, context);
        frame.render_widget(env.right_aligned(), at(columns));
    }
}

fn cells<'a>(theme: Theme, runs: &[Run<'a>], columns: Cols, role: Role) -> Line<'a> {
    theme.cell(&fit(runs, usize::from(columns.len)), role)
}

/// The prompt row is a strip with the same margin at each side as the selection bar. On it, one
/// column in, is the query, or the placeholder while the query is empty, and the host count at
/// the right end. The count goes when the query — or, with an empty filter, the placeholder —
/// would not keep the cursor's cell and a two-column gap before it.
/// `tail` is for a block with no host row: the selected host, in the count's place.
pub(super) fn prompt_row(
    frame: &mut Frame,
    area: Rect,
    view: &View<'_>,
    theme: Theme,
    tail: Option<(String, Role)>,
) {
    let strip = Rect::new(
        area.x.saturating_add(MARGIN),
        area.y,
        area.width.saturating_sub(2 * MARGIN),
        area.height,
    )
    .intersection(area);
    frame
        .buffer_mut()
        .set_style(strip, theme.style(Role::Field));

    let right = match tail {
        Some((text, role)) => Some((text, role)),
        None => {
            let text = counter(view);
            let typed = if view.query.is_empty() {
                PLACEHOLDER
            } else {
                view.query
            };
            let room = usize::from(strip.width)
                > usize::from(PADDING + GAP + PADDING) + columns(&text) + columns(typed);
            room.then_some((text, Role::Placeholder))
        }
    };
    let trailing = right
        .as_ref()
        .map_or(0, |(text, _)| width(text) + PADDING + GAP);
    let input = Rect::new(
        strip.x.saturating_add(PADDING),
        strip.y,
        strip.width.saturating_sub(PADDING + trailing),
        1,
    )
    .intersection(strip);
    if let Some((text, role)) = right {
        let at = Rect::new(
            strip.right().saturating_sub(PADDING + width(&text)),
            strip.y,
            width(&text),
            1,
        )
        .intersection(strip);
        frame.render_widget(span_line(theme, text, role), at);
    }
    let typed = width(view.query);
    let scroll = typed.saturating_sub(input.width.saturating_sub(1));
    if view.query.is_empty() {
        let text = PLACEHOLDER.to_owned();
        frame.render_widget(span_line(theme, text, Role::Placeholder), input);
    } else {
        let query = Paragraph::new(view.query)
            .style(theme.style(Role::Typed))
            .scroll((0, scroll));
        frame.render_widget(query, input);
    }
    frame.set_cursor_position((input.x.saturating_add(typed - scroll), input.y));
}

/// Where the selected host goes at the left of the row, and `Keys ?`, `Quit esc` and `Connect ↵`
/// at its right: a label, then its key in a keycap. The route takes the columns the caps leave.
pub(super) fn hint_row(
    frame: &mut Frame,
    area: Rect,
    view: &View<'_>,
    theme: Theme,
    slot: Option<&Slot>,
) {
    let inside = Rect::new(
        area.x.saturating_add(MARGIN),
        area.y,
        area.width.saturating_sub(2 * MARGIN),
        area.height,
    )
    .intersection(area);
    let Hint { caps } = hint(view, usize::from(inside.width));
    let route = slot.and_then(route::text);
    if let Some((text, role)) = route {
        let room = inside
            .width
            .saturating_sub(PADDING + GAP + u16::try_from(spread(&caps)).unwrap_or(u16::MAX));
        let at = Rect::new(inside.x.saturating_add(PADDING), inside.y, room, 1).intersection(area);
        frame.render_widget(
            span_line(theme, cut(&text, usize::from(at.width)), role),
            at,
        );
    }
    let gap = " ".repeat(HINT_GAP);
    let mut spans = Vec::new();
    for (at, cap) in caps.iter().enumerate() {
        if at > 0 {
            spans.push(theme.span(gap.clone(), Role::Fg));
        }
        spans.push(theme.span(cap.label, Role::Muted));
        spans.push(theme.span(format!(" {} ", cap.key), Role::Keycap));
    }
    frame.render_widget(Line::from(spans).right_aligned(), inside);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::testing::entries;

    #[test]
    fn widths_count_display_columns_not_chars() {
        let table = HostTable::new(&entries("ui"));
        assert_eq!((table.env, table.host, table.what), (11, 14, 22));
    }
}
