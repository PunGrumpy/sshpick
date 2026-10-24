use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{HighlightSpacing, Paragraph, Row as TableRow, Table, TableState};
use sshpick_core::{Entry, View};

use super::style::{BOLD, DIM, REVERSED, cell};
use crate::text::{ColumnWidths, TITLES, fit, plural, width};

const MARKER: &str = "▌ ";
const GUTTER: &str = "  ";
const COLUMN_SPACING: u16 = 1;

const PLACEHOLDER: &str = "type to filter";
const NO_MATCH: &str = "no match";

pub(super) const FOOTER_ROWS: u16 = 2;
const SELECT_HINT: &str = "↑ ↓ - Select";
const MORE_BINDS_ON_AN_EMPTY_QUERY: &str = "? - More binds";
const MORE_BINDS: &str = "F1 - More binds";
const HINT_GAP: &str = "   ";

#[derive(Debug)]
pub(super) struct HostTable {
    env: u16,
    host: u16,
}

impl HostTable {
    pub(super) fn new(entries: &[Entry]) -> HostTable {
        let widths = ColumnWidths::of(entries);
        let cells = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        HostTable {
            env: widths.env.map_or(0, cells),
            host: cells(widths.host),
        }
    }

    pub(super) fn list(&self, frame: &mut Frame, area: Rect, view: &View<'_>) {
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
            .row_highlight_style(REVERSED.remove_modifier(Modifier::DIM));
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
    frame.render_widget(Span::styled(">", BOLD), marker);
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

pub(super) fn footer(frame: &mut Frame, area: Rect, view: &View<'_>, warnings: usize) {
    let lines = footer_rows(view, warnings)
        .into_iter()
        .map(|row| Line::from(Span::styled(row, DIM)));
    frame.render_widget(Paragraph::new(lines.collect::<Vec<_>>()), area);
}

fn footer_rows(view: &View<'_>, warnings: usize) -> [String; 2] {
    let mut select = SELECT_HINT.to_owned();
    if view.below > 0 {
        select.push_str(&format!("{HINT_GAP}{} more below", view.below));
    }
    let mut binds = if view.query.is_empty() {
        MORE_BINDS_ON_AN_EMPTY_QUERY
    } else {
        MORE_BINDS
    }
    .to_owned();
    if warnings > 0 {
        binds.push_str(&format!(
            "{HINT_GAP}{warnings} {}",
            plural(warnings, "warning")
        ));
    }
    [select, binds]
}

#[cfg(test)]
mod tests {
    use sshpick_core::Picker;

    use super::*;
    use crate::tui::testing::entries;

    fn rows(query: &str, height: usize, warnings: usize) -> [String; 2] {
        let mut picker = Picker::new(entries("ui"), query);
        picker.follow(height);
        footer_rows(&picker.view(height), warnings)
    }

    #[test]
    fn the_footer_says_how_to_move_and_where_the_rest_of_the_keys_are() {
        assert_eq!(rows("", 10, 0), ["↑ ↓ - Select", "? - More binds"]);
        assert_eq!(
            rows("consul", 10, 0),
            ["↑ ↓ - Select", "F1 - More binds"],
            "? is a character once there is a query"
        );
    }

    #[test]
    fn the_footer_counts_matches_below_and_warnings() {
        assert_eq!(rows("", 3, 0)[0], "↑ ↓ - Select   2 more below");
        assert_eq!(rows("", 5, 0)[0], "↑ ↓ - Select");
        assert_eq!(rows("", 10, 1)[1], "? - More binds   1 warning");
        assert_eq!(rows("x", 10, 3)[1], "F1 - More binds   3 warnings");
    }

    #[test]
    fn widths_count_display_columns_not_chars() {
        let table = HostTable::new(&entries("ui"));
        assert_eq!((table.env, table.host), (11, 14));
        assert_eq!(HostTable::new(&[]).host, 4, "never narrower than its title");
    }
}
