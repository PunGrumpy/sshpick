use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sshpick_core::{BINDINGS, Run};

use super::style::DIM;
use crate::text::{columns, fit};

const INDENT: &str = "  ";
const DASH: &str = " - ";

pub(super) fn rows() -> u16 {
    u16::try_from(1 + BINDINGS.len()).unwrap_or(u16::MAX)
}

pub(super) fn help(frame: &mut Frame, area: Rect) {
    let room = usize::from(area.height);
    if room == 0 {
        return;
    }
    let width = usize::from(area.width);
    let shown = if room > BINDINGS.len() {
        BINDINGS.len()
    } else {
        room.saturating_sub(2)
    };
    let hidden = BINDINGS.len() - shown;
    let keys = BINDINGS
        .iter()
        .map(|binding| columns(binding.keys))
        .max()
        .unwrap_or(0);

    let mut lines = Vec::new();
    if room > 1 || hidden == 0 {
        lines.push(Line::default());
    }
    for binding in &BINDINGS[..shown] {
        let pad = " ".repeat(keys - columns(binding.keys));
        let text = format!("{INDENT}{}{pad}{DASH}{}", binding.keys, binding.does);
        lines.push(Line::raw(cut(&text, width)));
    }
    if hidden > 0 {
        let text = format!("{INDENT}+{hidden} more · sshpick pick --help");
        lines.push(Line::from(Span::styled(cut(&text, width), DIM)));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn cut(text: &str, width: usize) -> String {
    let whole = Run {
        text,
        matched: false,
    };
    fit(&[whole], width).iter().map(|run| run.text).collect()
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use sshpick_core::{BINDINGS, Key};
    use test_case::test_case;

    use crate::tui::testing::{cell, dim, frame, ready, render, setup, terminal};

    fn open(picker: &mut sshpick_core::Picker) {
        assert_eq!(picker.handle(Key::F(1)), None);
        assert!(picker.help_open());
    }

    #[test]
    fn the_help_grows_the_block_under_the_route_row_with_every_binding() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let plain = render(&mut terminal, &screen, &mut picker, &ready());
        open(&mut picker);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("help_80x24", frame(&lines));

        assert_eq!(lines.len(), plain.len() + BINDINGS.len() + 1);
        assert_eq!(
            lines[..8],
            plain[..8],
            "the prompt, header, hosts and route stay"
        );
        assert!(lines[8].is_empty(), "a blank row opens the section");
        for (row, binding) in lines[9..19].iter().zip(BINDINGS) {
            assert!(
                row.starts_with(&format!("  {}", binding.keys)) && row.ends_with(binding.does),
                "{row:?} for {binding:?}"
            );
        }
        assert_eq!(lines[19], "  ↑↓ move   enter ssh   esc close");
        assert!(
            !lines.concat().contains(['│', '─', '┌', '┐', '└', '┘']),
            "a section of the block, not a box over it"
        );
        let plain_keys = (2..60).all(|x| !dim(&cell(&terminal, x, 9)));
        assert!(
            plain_keys,
            "the key rows are plain; only the hint row is dim"
        );
        assert!(dim(&cell(&terminal, 2, 19)));
    }

    #[test]
    fn the_help_is_non_modal_so_the_list_still_follows_what_is_typed() {
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let mut terminal = terminal(80, 24);
        for c in "consul".chars() {
            assert_eq!(picker.handle(Key::Char(c)), None);
        }
        assert!(picker.help_open(), "typing does not close it");
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert!(lines[0].starts_with("> consul") && lines[0].ends_with("2/5"));
        assert!(lines[2].starts_with("▌ NEW STAGING newstg-consul1"));
        assert!(lines[9].starts_with("  Enter"), "{lines:#?}");
        assert_eq!(lines[19], "  ↑↓ move   enter ssh   esc close");
    }

    #[test]
    fn closing_the_help_gives_the_rows_back() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let plain = render(&mut terminal, &screen, &mut picker, &ready());
        open(&mut picker);
        let with_help = render(&mut terminal, &screen, &mut picker, &ready());
        assert!(with_help.len() > plain.len());
        assert_eq!(picker.handle(Key::Esc), None);
        assert_eq!(render(&mut terminal, &screen, &mut picker, &ready()), plain);
    }

    #[test]
    fn a_short_terminal_cuts_the_keys_and_says_where_the_rest_are() {
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let mut terminal = terminal(80, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("help_80x12", frame(&lines));
        assert_eq!(lines.len(), 11, "all but one row of the terminal");
        assert_eq!(lines[9], "  +8 more · sshpick pick --help");
        assert!(dim(&cell(&terminal, 2, 9)) && dim(&cell(&terminal, 30, 9)));
        assert_eq!(
            lines[10],
            format!("{:<77}1–3", "  ↑↓ move   enter ssh   esc close"),
            "the list kept three rows, so the range shows"
        );
    }

    #[test_case(24, 0, 11 ; "every key fits")]
    #[test_case(21, 0, 11 ; "the shortest terminal that takes the whole block")]
    #[test_case(19, 0, 11 ; "the list gave up two hosts")]
    #[test_case(18, 2, 10 ; "two keys short")]
    #[test_case(15, 5, 7 ; "half of them")]
    #[test_case(12, 8, 4 ; "the 80x12 terminal")]
    #[test_case(10, 10, 2 ; "only the blank row and the pointer")]
    #[test_case(9, 10, 1 ; "only the pointer")]
    fn the_list_keeps_three_rows_and_the_help_gets_the_rest(
        height: u16,
        hidden: usize,
        rows: usize,
    ) {
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let lines = render(&mut terminal(80, height), &screen, &mut picker, &ready());
        let hint = lines.len() - 1;
        let section = &lines[hint - rows..hint];
        let pointer = format!("  +{hidden} more · sshpick pick --help");
        assert_eq!(
            section.last().is_some_and(|row| *row == pointer),
            hidden > 0,
            "{section:#?}"
        );
        let keys = section
            .iter()
            .filter(|row| {
                BINDINGS
                    .iter()
                    .any(|b| row.starts_with(&format!("  {}", b.keys)))
            })
            .count();
        assert_eq!(keys, BINDINGS.len() - hidden, "{section:#?}");

        let list = lines.len() - 4 - rows;
        let hosts = lines[2..2 + list]
            .iter()
            .filter(|row| !row.is_empty())
            .count();
        assert_eq!(hosts, list.min(5), "the list gave the rows up");
        assert!(list >= 3, "but kept three");
        assert!(
            lines[2 + list].starts_with("  → "),
            "the route row is under the list"
        );
    }

    #[test]
    fn with_no_room_the_help_draws_nothing_and_the_list_keeps_its_rows() {
        let (screen, mut picker) = setup("ui", "");
        let plain = render(&mut terminal(80, 7), &screen, &mut picker, &ready());
        open(&mut picker);
        let lines = render(&mut terminal(80, 7), &screen, &mut picker, &ready());
        assert_eq!(lines.len(), plain.len());
        assert_eq!(lines[..5], plain[..5]);
        assert!(
            lines[5].starts_with("  ↑↓ move   enter ssh   esc close") && lines[5].ends_with("1–3"),
            "{:?}",
            lines[5]
        );
    }

    #[test]
    fn the_help_is_cut_to_the_row_at_any_size() {
        let (screen, mut picker) = setup("ui", "consul");
        open(&mut picker);
        let sizes = (0..=45)
            .map(|width| (width, 14))
            .chain([(99, 24), (100, 24), (140, 40)])
            .flat_map(|(width, most)| (0..=most).map(move |height| (width, height)));
        for (width, height) in sizes {
            let lines = render(&mut terminal(width, height), &screen, &mut picker, &ready());
            assert!(
                lines
                    .iter()
                    .all(|line| line.chars().count() <= usize::from(width)),
                "{width}x{height}"
            );
            assert!(lines.len() < usize::from(height).max(2), "{width}x{height}");
        }
        let lines = render(&mut terminal(40, 24), &screen, &mut picker, &ready());
        assert!(
            lines[9..19].iter().any(|line| line.ends_with('…')),
            "a long key row ends in an ellipsis: {lines:#?}"
        );
    }

    #[test]
    fn a_wide_terminal_gets_the_same_rows_with_more_room() {
        let (screen, mut picker) = setup("ui", "consul");
        open(&mut picker);
        let lines = render(&mut terminal(120, 30), &screen, &mut picker, &ready());
        assert_snapshot!("help_120x30", frame(&lines));
    }
}
