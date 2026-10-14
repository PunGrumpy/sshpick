use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{HighlightSpacing, Paragraph, Row as TableRow, Table, TableState};
use sshpick_core::{Entry, View};

use super::style::{DIM, MATCHED, cell};
use crate::text::{fit, width};

const TITLES: [&str; 3] = ["ENV", "HOST", "WHAT"];

const MARKER: &str = "▌ ";
const GUTTER: &str = "  ";
const COLUMN_SPACING: u16 = 1;

const PLACEHOLDER: &str = "type to filter";
const NO_MATCH: &str = "no match";

/// The column widths of the host table, measured once from every entry so the columns do not
/// move as the query narrows the list.
#[derive(Debug)]
pub(super) struct HostTable {
    env: u16,
    host: u16,
}

impl HostTable {
    pub(super) fn new(entries: &[Entry]) -> HostTable {
        let widest = |text: fn(&Entry) -> &str| {
            entries
                .iter()
                .map(|entry| width(text(entry)))
                .max()
                .unwrap_or(0)
        };
        let env = widest(|entry| &entry.env);
        HostTable {
            env: if env == 0 {
                0
            } else {
                env.max(width(TITLES[0]))
            },
            host: widest(|entry| entry.host.as_str()).max(width(TITLES[1])),
        }
    }

    pub(super) fn list(&self, frame: &mut Frame, area: Rect, view: &View<'_>, selected: Style) {
        if view.matched == 0 {
            if view.total > 0 {
                frame.render_widget(Line::styled(format!("{GUTTER}{NO_MATCH}"), DIM), area);
            }
            return;
        }
        let what_width = self.what_width(area.width);
        let table_rows = view.rows.iter().map(|row| {
            let what = fit(&row.what, what_width);
            let columns = [
                (self.env > 0).then(|| cell(&row.env, DIM)),
                Some(cell(&row.host, Style::new())),
                Some(cell(&what, DIM)),
            ];
            TableRow::new(columns.into_iter().flatten())
        });
        let table = Table::new(table_rows, self.widths())
            .column_spacing(COLUMN_SPACING)
            .highlight_symbol(MARKER)
            .highlight_spacing(HighlightSpacing::Always)
            .row_highlight_style(selected.remove_modifier(Modifier::DIM));
        let selected = view.rows.iter().position(|row| row.selected);
        frame.render_stateful_widget(table, area, &mut TableState::new().with_selected(selected));
    }

    pub(super) fn header(&self, frame: &mut Frame, area: Rect) {
        let titles = TITLES.into_iter().skip(usize::from(self.env == 0));
        let table = Table::new(Vec::<TableRow>::new(), self.widths())
            .header(TableRow::new(titles).style(DIM))
            .column_spacing(COLUMN_SPACING)
            .highlight_symbol(GUTTER)
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_widget(table, area);
    }

    fn widths(&self) -> Vec<Constraint> {
        let env = (self.env > 0).then_some(Constraint::Length(self.env));
        env.into_iter()
            .chain([Constraint::Length(self.host), Constraint::Fill(1)])
            .collect()
    }

    // The Table resolves its columns the same way; the cut must land on the column's edge.
    fn what_width(&self, total: u16) -> usize {
        let [_, columns] =
            Layout::horizontal([Constraint::Length(width(MARKER)), Constraint::Fill(0)])
                .areas(Rect::new(0, 0, total, 1));
        Layout::horizontal(self.widths())
            .spacing(COLUMN_SPACING)
            .split(columns)
            .last()
            .map_or(0, |what| usize::from(what.width))
    }
}

pub(super) fn prompt_row(frame: &mut Frame, area: Rect, view: &View<'_>) {
    let count = format!("{}/{}", view.matched, view.total);
    let [marker, rest] =
        Layout::horizontal([Constraint::Length(2), Constraint::Fill(1)]).areas(area);
    let [input, tally] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(width(&count) + 1)])
            .areas(rest);
    frame.render_widget(Span::styled(">", MATCHED), marker);
    frame.render_widget(Line::styled(count, DIM).right_aligned(), tally);
    let typed = width(view.query);
    let scroll = typed.saturating_sub(input.width.saturating_sub(1));
    if view.query.is_empty() {
        frame.render_widget(Span::styled(PLACEHOLDER, DIM), input);
    } else {
        frame.render_widget(Paragraph::new(view.query).scroll((0, scroll)), input);
    }
    frame.set_cursor_position((input.x.saturating_add(typed - scroll), input.y));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::testing::entries;

    #[test]
    fn widths_count_display_columns_not_chars() {
        let table = HostTable::new(&entries("ui"));
        assert_eq!((table.env, table.host), (11, 14));
        assert_eq!(HostTable::new(&[]).host, 4, "never narrower than its title");
    }
}
