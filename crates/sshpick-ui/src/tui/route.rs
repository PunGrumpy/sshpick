//! Values come from ssh and may hold control characters: they reach the buffer through `Span`,
//! which drops them.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders};
use sshpick_core::{Host, Route, Slot};

use super::style::DIM;
use crate::text::{columns, fit_parts};

const INDENT: &str = "  ";
const ARROW: &str = "→";
const RESOLVING: &str = "resolving…";
const VIA: &str = "via";
const RUNS: &str = "runs";
const PLAIN: Style = Style::new();
/// Between the longest key and its value, as `sshpick inspect` pads them.
const KEY_GAP: usize = 2;

pub(super) fn row(frame: &mut Frame, area: Rect, slot: Option<&Slot>) {
    let Some(slot) = slot else {
        return;
    };
    let (target, headline);
    let mut parts = vec![(INDENT, PLAIN)];
    match slot {
        Slot::Waiting => parts.extend([(ARROW, DIM), (" ", DIM), (RESOLVING, DIM)]),
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
    frame.render_widget(
        spans(&fit_parts(&parts, usize::from(area.width), PLAIN)),
        area,
    );
}

/// The route beside the list: a dim rule, the host, then the rows `sshpick inspect` prints under
/// Resolved, `resolving…`, or why ssh -G failed in as many rows as it takes.
pub(super) fn panel(frame: &mut Frame, area: Rect, selected: Option<(&Host, &Slot)>) {
    frame.render_widget(Block::new().borders(Borders::LEFT).border_style(DIM), area);
    let [_, body] = Layout::horizontal([Constraint::Length(2), Constraint::Fill(1)]).areas(area);
    let Some((host, slot)) = selected else {
        return;
    };
    let width = usize::from(body.width);
    let mut lines = vec![spans(&fit_parts(&[(host.as_str(), PLAIN)], width, PLAIN))];
    match slot {
        Slot::Waiting => lines.push(Line::styled(RESOLVING, DIM)),
        Slot::Failed(error) => {
            let room = body.height.saturating_sub(1);
            lines.extend(
                wrap(&error.headline(), width, room)
                    .into_iter()
                    .map(Line::raw),
            );
        }
        Slot::Ready(preview) => {
            if let Some(route) = preview.route() {
                lines.extend(resolved(route, width));
            }
        }
    }
    frame.render_widget(Text::from(lines), body);
}

fn resolved(route: Route<'_>, width: usize) -> Vec<Line<'_>> {
    let rows: Vec<(&str, &str)> = [
        ("user", route.user),
        ("hostname", Some(route.hostname)),
        ("port", route.port),
        ("proxyjump", route.via),
        ("remotecommand", route.runs),
    ]
    .into_iter()
    .filter_map(|(key, value)| Some((key, value?)))
    .collect();
    let keys = rows.iter().map(|(key, _)| key.len()).max().unwrap_or(0) + KEY_GAP;
    rows.into_iter()
        .map(|(key, value)| {
            let key = Span::styled(format!("{key:<keys$}"), DIM);
            let value = fit_parts(&[(value, PLAIN)], width.saturating_sub(keys), PLAIN);
            let value = value
                .into_iter()
                .map(|(text, style)| Span::styled(text, style));
            std::iter::once(key).chain(value).collect()
        })
        .collect()
}

/// `text` in at most `rows` lines of `width` columns, broken between words where it can be. The
/// last line ends in `…` when some is left over.
fn wrap(text: &str, width: usize, rows: u16) -> Vec<String> {
    let rows = usize::from(rows);
    if width == 0 || rows == 0 {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for mut word in text.split(' ').filter(|word| !word.is_empty()) {
        loop {
            let used = if line.is_empty() {
                0
            } else {
                columns(&line) + 1
            };
            if used + columns(word) <= width {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
                break;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            let (head, rest) = word.split_at(head(word, width));
            lines.push(head.to_owned());
            word = rest;
            if word.is_empty() {
                break;
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > rows {
        let rest = lines[rows - 1..].join(" ");
        lines.truncate(rows - 1);
        lines.push(
            fit_parts(&[(rest.as_str(), ())], width, ())
                .into_iter()
                .map(|(text, _)| text)
                .collect(),
        );
    }
    lines
}

/// The byte length of the longest head of `word` that fits in `width` columns, and at least one
/// character, so that wrapping always moves on.
fn head(word: &str, width: usize) -> usize {
    let mut used = 0;
    for (start, c) in word.char_indices() {
        used += columns(&word[start..start + c.len_utf8()]);
        if used > width {
            return if start == 0 { c.len_utf8() } else { start };
        }
    }
    word.len()
}

fn spans<'a>(parts: &[(&'a str, Style)]) -> Line<'a> {
    parts
        .iter()
        .map(|&(text, style)| Span::styled(text, style))
        .collect()
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    //          text                   width rows  lines
    #[test_case("a bb ccc", 4, 3, &["a bb", "ccc"]; "between words")]
    #[test_case("a  bb", 9, 3, &["a bb"]; "runs of spaces are one")]
    #[test_case("abcdefghij", 4, 3, &["abcd", "efgh", "ij"]; "a word wider than the row is split")]
    #[test_case("aa bb cc dd ee", 5, 2, &["aa bb", "cc d…"]; "what is left over ends in an ellipsis")]
    #[test_case("日本語です", 4, 3, &["日本", "語で", "す"]; "by display columns")]
    #[test_case("日本", 1, 3, &["日", "本"]; "a character wider than the row still moves on")]
    #[test_case("abc", 0, 3, &[]; "no columns")]
    #[test_case("abc", 3, 0, &[]; "no rows")]
    fn wrap_breaks_between_words_and_cuts_what_does_not_fit(
        text: &str,
        width: usize,
        rows: u16,
        want: &[&str],
    ) {
        assert_eq!(wrap(text, width, rows), want);
    }
}
