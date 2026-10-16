use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Wrap};
use sshpick_core::{Host, Slot};

use super::style::DIM;
use crate::text::{columns, fit};

const STACKED_INDENT: u16 = 2;
const KEY_WIDTH: usize = 14;
const FOOTER_INDENT: &str = "   ";
const FOOTER_GAP: &str = "   ";
const ENTER_HINT: &str = "Enter - ssh";
const ESC_HINT: &str = "Esc - Quit";

pub(super) type Pane<'a> = Option<(&'a Host, &'a Slot)>;

/// `host > ssh -G`, with a `»` while ssh has not answered and `failed` when it did not.
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

/// The preview under the list: a dim title row, then five rows of body.
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

/// The preview beside the list: a borderless block whose dim title is the host and whose dim
/// footer is what Enter and Esc do. With no selection the title and body are gone and the
/// footer stays.
pub(super) fn side(frame: &mut Frame, area: Rect, pane: Pane<'_>, matched: usize) {
    let mut block = Block::new().title_bottom(footer(matched, usize::from(area.width)));
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

/// `   Enter - ssh   Esc - Quit`, without Enter when nothing matches, and with hints dropped from
/// the right until it fits.
fn footer(matched: usize, width: usize) -> Line<'static> {
    let hints: Vec<&str> = [(matched > 0).then_some(ENTER_HINT), Some(ESC_HINT)]
        .into_iter()
        .flatten()
        .collect();
    let fits = |count: usize| {
        let text = format!("{FOOTER_INDENT}{}", hints[..count].join(FOOTER_GAP));
        (columns(&text) <= width).then_some(text)
    };
    let text = (1..=hints.len()).rev().find_map(fits).unwrap_or_default();
    Line::styled(text, DIM)
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
    fn the_footer_names_enter_and_esc_and_drops_hints_from_the_right() {
        let line = |matched, width| text(&footer(matched, width));
        assert_eq!(line(5, 40), "   Enter - ssh   Esc - Quit");
        assert_eq!(line(5, 27), "   Enter - ssh   Esc - Quit");
        assert_eq!(line(5, 26), "   Enter - ssh");
        assert_eq!(line(5, 14), "   Enter - ssh");
        assert_eq!(line(5, 13), "");
        assert_eq!(line(0, 40), "   Esc - Quit", "nothing to ssh to");
        assert_eq!(line(0, 13), "   Esc - Quit");
        assert_eq!(line(0, 12), "");
        assert_eq!(footer(5, 40).style, DIM);
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
