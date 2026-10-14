use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use sshpick_core::{Host, Slot};

use super::style::{DIM, cell};
use crate::text::{columns, fit};

// A space, the bar, a space.
const DIVIDER: &str = " │";
const DIVIDER_WIDTH: u16 = 3;
const STACKED_INDENT: u16 = 2;
const KEY_WIDTH: usize = 14;

pub(super) type Pane<'a> = Option<(&'a Host, &'a Slot)>;

pub(super) fn title(host: &Host, slot: &Slot) -> String {
    let failed = if matches!(slot, Slot::Failed(_)) {
        " failed"
    } else {
        ""
    };
    format!("ssh -G {host}{failed}")
}

pub(super) fn stacked(frame: &mut Frame, area: Rect, pane: Pane<'_>) {
    let [_, body] =
        Layout::horizontal([Constraint::Length(STACKED_INDENT), Constraint::Fill(1)]).areas(area);
    if let Some((_, slot)) = pane {
        slot_body(frame, body, slot, None);
    }
}

pub(super) fn side(frame: &mut Frame, area: Rect, pane: Pane<'_>) {
    let [divider, content] =
        Layout::horizontal([Constraint::Length(DIVIDER_WIDTH), Constraint::Fill(1)]).areas(area);
    let bars = vec![Line::styled(DIVIDER, DIM); usize::from(area.height)];
    frame.render_widget(Paragraph::new(bars), divider);
    let Some((host, slot)) = pane else {
        return;
    };
    let [heading, body] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(content);
    let heading_text = title(host, slot);
    let heading_runs = fit(&[(&heading_text, false)], usize::from(heading.width));
    frame.render_widget(cell(&heading_runs, DIM), heading);
    let room = usize::from(body.width).saturating_sub(KEY_WIDTH);
    slot_body(frame, body, slot, Some(room));
}

fn slot_body(frame: &mut Frame, area: Rect, slot: &Slot, room: Option<usize>) {
    match slot {
        Slot::Waiting => {}
        Slot::Ready(preview) => {
            frame.render_widget(Paragraph::new(key_values(preview.lines(), room)), area);
        }
        Slot::Failed(error) => {
            let text = Paragraph::new(error.to_string()).wrap(Wrap { trim: false });
            frame.render_widget(text, area);
        }
    }
}

fn key_values<'a>(
    lines: impl Iterator<Item = (&'static str, &'a str)>,
    room: Option<usize>,
) -> Text<'a> {
    lines
        .map(|(key, value)| {
            let gap = KEY_WIDTH.saturating_sub(columns(key)).max(1);
            let key = format!("{key}{}", " ".repeat(gap));
            let value = match room {
                Some(room) => fit(&[(value, false)], room),
                None => vec![(value, false)],
            };
            let mut spans = vec![Span::styled(key, DIM)];
            spans.extend(value.into_iter().map(|(text, _)| Span::raw(text)));
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::*;
    use crate::tui::testing::text;

    #[test]
    fn preview_rows_pad_keys_dim_and_keep_a_gap_after_long_ones() {
        let rows = key_values(
            [
                ("user", "ubuntu"),
                ("remotecommand", "sudo ssh -t host"),
                ("proxyjump", "bastion"),
            ]
            .into_iter(),
            None,
        );
        let lines: Vec<String> = rows.lines.iter().map(text).collect();
        assert_eq!(
            lines,
            [
                "user          ubuntu",
                "remotecommand sudo ssh -t host",
                "proxyjump     bastion"
            ]
        );
        for line in &rows.lines {
            assert_eq!(line.spans[0].style, DIM);
            assert_eq!(line.spans[1].style, Style::default());
        }
    }

    #[test]
    fn preview_values_wider_than_the_room_end_in_an_ellipsis() {
        let rows = key_values(
            [
                ("user", "ubuntu"),
                ("proxyjump", "a-very-long-bastion-name"),
            ]
            .into_iter(),
            Some(10),
        );
        let lines: Vec<String> = rows.lines.iter().map(text).collect();
        assert_eq!(lines, ["user          ubuntu", "proxyjump     a-very-lo…"]);
    }
}
