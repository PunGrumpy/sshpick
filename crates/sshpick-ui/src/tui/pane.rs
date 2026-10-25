use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use sshpick_core::{Host, Run, Slot};

use super::style::DIM;
use crate::text::{columns, fit};

const STACKED_INDENT: u16 = 2;
const KEY_WIDTH: usize = 14;

pub(super) type Pane<'a> = Option<(&'a Host, &'a Slot)>;

pub(super) fn title(host: &Host, slot: &Slot) -> String {
    let status = match slot {
        Slot::Waiting => " »",
        Slot::Ready(_) => "",
        Slot::Failed(_) => " failed",
    };
    format!("{host} > ssh -G{status}")
}

fn heading(host: &Host, slot: &Slot) -> Line<'static> {
    Line::styled(format!(" {} ", title(host, slot)), DIM)
}

pub(super) fn stacked(frame: &mut Frame, area: Rect, pane: Pane<'_>) {
    let Some((host, slot)) = pane else {
        return;
    };
    let block = Block::new().title(heading(host, slot));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [_, body] =
        Layout::horizontal([Constraint::Length(STACKED_INDENT), Constraint::Fill(1)]).areas(inner);
    slot_body(frame, body, slot, None);
}

pub(super) fn side(frame: &mut Frame, area: Rect, pane: Pane<'_>) {
    let mut block = Block::new();
    if let Some((host, slot)) = pane {
        block = block.title(heading(host, slot));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if let Some((_, slot)) = pane {
        let room = usize::from(inner.width).saturating_sub(KEY_WIDTH);
        slot_body(frame, inner, slot, Some(room));
    }
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
            let value = [Run {
                text: value,
                matched: false,
            }];
            let value = match room {
                Some(room) => fit(&value, room),
                None => value.to_vec(),
            };
            let mut spans = vec![Span::styled(key, DIM)];
            spans.extend(value.into_iter().map(|run| Span::raw(run.text)));
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;
    use sshpick_core::{Preview, ResolveError};

    use super::*;
    use crate::tui::testing::{entries, text};

    fn host() -> Host {
        entries("ui").remove(0).host
    }

    #[test]
    fn the_title_is_the_host_then_ssh_dash_g_then_how_it_went() {
        let host = host();
        assert_eq!(
            title(&host, &Slot::Ready(Preview::parse(""))),
            "lab-a > ssh -G"
        );
        assert_eq!(title(&host, &Slot::Waiting), "lab-a > ssh -G »");
        let failed = Slot::Failed(ResolveError::Timeout);
        assert_eq!(title(&host, &failed), "lab-a > ssh -G failed");
    }

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
