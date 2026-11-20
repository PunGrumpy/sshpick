use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use sshpick_core::{BINDINGS, Binding};

use super::style::{Role, Theme};
use super::table::{MARGIN, PADDING};
use crate::text::{columns, cut};

/// The help's keys start where the host names do.
const INDENT: &str = "  ";
const GAP: &str = "  ";

pub(super) fn rows() -> u16 {
    u16::try_from(1 + BINDINGS.len()).unwrap_or(u16::MAX)
}

fn key_column() -> usize {
    BINDINGS
        .iter()
        .map(|binding| columns(binding.keys))
        .max()
        .unwrap_or(0)
}

/// `  keys  does`: the key column, padded to the widest, two columns, then the description.
fn row(theme: Theme, binding: &Binding, keys: usize, room: usize) -> Line<'static> {
    debug_assert_eq!(INDENT.len(), usize::from(MARGIN + PADDING));
    let pad = " ".repeat(keys - columns(binding.keys) + GAP.len());
    let head = format!("{INDENT}{}{pad}", binding.keys);
    let head_cols = columns(&head);
    if head_cols >= room {
        return Line::from(theme.span(cut(&head, room), Role::Fg));
    }
    Line::from(vec![
        theme.span(head, Role::Fg),
        theme.span(cut(binding.does, room - head_cols), Role::Muted),
    ])
}

pub(super) fn help(frame: &mut Frame, area: Rect, theme: Theme) {
    let room = usize::from(area.height);
    if room == 0 {
        return;
    }
    let room_across = usize::from(area.width.saturating_sub(MARGIN));
    let shown = if room > BINDINGS.len() {
        BINDINGS.len()
    } else {
        room.saturating_sub(2)
    };
    let hidden = BINDINGS.len() - shown;
    let keys = key_column();

    let mut lines = Vec::new();
    if room > 1 || hidden == 0 {
        lines.push(Line::default());
    }
    for binding in &BINDINGS[..shown] {
        lines.push(row(theme, binding, keys, room_across));
    }
    if hidden > 0 {
        let text = format!("{INDENT}+{hidden} more · sshpick pick --help");
        lines.push(Line::from(theme.span(cut(&text, room_across), Role::Muted)));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use sshpick_core::{BINDINGS, Key};
    use test_case::test_case;

    use crate::tui::testing::{
        cell, is_muted, is_plain, ready, render, screen_lines, setup, shown, terminal,
    };

    fn open(picker: &mut sshpick_core::Picker) {
        assert_eq!(picker.handle(Key::F(1)), None);
        assert!(picker.help_open());
    }

    #[test]
    fn the_help_lists_every_binding_above_the_rule_in_the_same_panel() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let plain = render(&mut terminal, &screen, &mut picker, &ready());
        let outer = screen_lines(&terminal);
        open(&mut picker);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("help_80x24", shown(&terminal));
        assert_eq!(
            screen_lines(&terminal)
                .iter()
                .filter(|row| row.contains('╭') || row.contains('╰') || row.contains('├'))
                .collect::<Vec<_>>(),
            outer
                .iter()
                .filter(|row| row.contains('╭') || row.contains('╰') || row.contains('├'))
                .collect::<Vec<_>>(),
            "the help does not move the panel"
        );
        assert_eq!(lines.len(), 20, "the panel's inside, border left out");
        assert_eq!(&lines[..2], &plain[..2], "the prompt and the gap stay");
        let words = |rows: &[String]| -> Vec<Vec<String>> {
            rows.iter()
                .map(|row| row.split_whitespace().map(str::to_owned).collect())
                .collect()
        };
        assert_eq!(words(&lines[2..7]), words(&plain[2..7]), "the hosts stay");
        assert!(lines[7].is_empty(), "a blank row opens the section");
        for (row, binding) in lines[8..18].iter().zip(BINDINGS) {
            assert!(
                row.starts_with(&format!("  {}", binding.keys)) && row.ends_with(binding.does),
                "{row:?} for {binding:?}"
            );
        }
        assert!(
            !lines.iter().any(|row| row.contains('…')),
            "no key row is cut at 80x24: {lines:#?}"
        );
        assert!(lines[18].starts_with('─'), "the rule is below the help");
        assert!(
            lines[19].ends_with("Close esc    Connect ↵"),
            "{:?}",
            lines[19]
        );
        assert!(
            !lines.concat().contains(['│', '┌', '┐', '┘']),
            "the help is a section of the panel, not a box inside it"
        );
    }

    #[test]
    fn the_keys_are_plain_and_the_descriptions_muted() {
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        let row = lines
            .iter()
            .position(|row| row.starts_with("  Enter"))
            .unwrap() as u16;
        assert!(
            (2..7).all(|x| is_plain(&cell(&terminal, x, row))),
            "the keys"
        );
        let does = u16::try_from(lines[usize::from(row)].find("ssh to").unwrap()).unwrap();
        assert!(
            (does..does + 6).all(|x| is_muted(&cell(&terminal, x, row), false)),
            "the description"
        );
    }

    #[test]
    fn a_terminal_too_narrow_for_the_help_cuts_only_the_rows_that_do_not_fit() {
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let mut terminal = terminal(60, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        let cut = |key: &str| {
            lines
                .iter()
                .any(|row| row.trim_start().starts_with(key) && row.contains('…'))
        };
        assert!(cut("Esc, Ctrl-C"), "the longest row is cut: {lines:#?}");
        assert!(!cut("Ctrl-U"), "a row that fits is whole: {lines:#?}");
        assert!(
            lines.iter().all(|row| row.chars().count() <= 54),
            "{lines:#?}"
        );
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
        assert!(lines[0].starts_with("  consul"));
        assert!(lines[2].starts_with("  newstg-consul1"));
        assert!(
            lines.iter().any(|row| row.starts_with("  Enter")),
            "{lines:#?}"
        );
        assert!(
            lines
                .last()
                .is_some_and(|row| row.ends_with("Close esc    Connect ↵")),
            "{:?}",
            lines.last()
        );
    }

    #[test]
    fn closing_the_help_gives_the_rows_back() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let plain = render(&mut terminal, &screen, &mut picker, &ready());
        open(&mut picker);
        let with_help = render(&mut terminal, &screen, &mut picker, &ready());
        assert_ne!(with_help, plain);
        assert_eq!(picker.handle(Key::Esc), None);
        assert_eq!(render(&mut terminal, &screen, &mut picker, &ready()), plain);
    }

    #[test]
    fn a_short_terminal_cuts_the_keys_and_says_where_the_rest_are() {
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let mut terminal = terminal(80, 16);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("help_80x16", shown(&terminal));
        assert_eq!(lines.len(), 12, "the whole inside of the panel");
        assert_eq!(lines[9], "  +7 more · sshpick pick --help");
        assert!(
            is_muted(&cell(&terminal, 2, 9), false) && is_muted(&cell(&terminal, 30, 9), false)
        );
        assert!(
            lines[11].ends_with("Close esc    Connect ↵"),
            "{:?}",
            lines[11]
        );
    }

    #[test_case(24, 0 ; "every key fits")]
    #[test_case(20, 3 ; "the list gave up lines")]
    #[test_case(17, 6 ; "six keys short")]
    #[test_case(16, 7 ; "half of them")]
    #[test_case(14, 9 ; "one key")]
    #[test_case(12, 10 ; "only the pointer")]
    #[test_case(8, 10 ; "unbordered, the same")]
    fn the_list_keeps_three_rows_and_the_help_gets_the_rest(height: u16, hidden: usize) {
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let lines = render(&mut terminal(60, height), &screen, &mut picker, &ready());
        let hint = lines.len() - 1;
        let rule = hint - usize::from(height >= 10);
        let section = &lines[2..rule];
        let pointer = format!("  +{hidden} more · sshpick pick --help");
        if hidden > 0 {
            assert_eq!(
                section.iter().rfind(|row| !row.is_empty()),
                Some(&pointer),
                "{section:#?}"
            );
        } else {
            assert!(!section.iter().any(|row| row.contains("+")), "{section:#?}");
        }
        let keys = section
            .iter()
            .filter(|row| {
                BINDINGS
                    .iter()
                    .any(|b| row.starts_with(&format!("  {}", b.keys)))
            })
            .count();
        assert_eq!(keys, BINDINGS.len() - hidden, "{section:#?}");
        assert!(
            lines[2].starts_with("  lab-a") && lines[3].starts_with("  lab-b"),
            "the list kept its rows: {lines:#?}"
        );
    }

    #[test]
    fn with_no_room_the_help_draws_nothing_and_the_list_keeps_its_rows() {
        let (screen, mut picker) = setup("ui", "");
        let plain = render(&mut terminal(60, 6), &screen, &mut picker, &ready());
        open(&mut picker);
        let lines = render(&mut terminal(60, 6), &screen, &mut picker, &ready());
        assert_eq!(lines.len(), plain.len());
        assert_eq!(lines[..5], plain[..5]);
        assert!(
            lines[5].ends_with('↵'),
            "the route takes the labels' room: {:?}",
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
            let mut terminal = terminal(width, height);
            render(&mut terminal, &screen, &mut picker, &ready());
            let lines = screen_lines(&terminal);
            assert!(
                lines
                    .iter()
                    .all(|line| line.chars().count() <= usize::from(width)),
                "{width}x{height}"
            );
            assert_eq!(lines.len(), usize::from(height.max(1)), "{width}x{height}");
        }
        let lines = render(&mut terminal(40, 20), &screen, &mut picker, &ready());
        assert!(
            lines.iter().any(|line| line.ends_with('…')),
            "a long key row ends in an ellipsis: {lines:#?}"
        );
    }

    #[test]
    fn a_wide_terminal_draws_the_help_in_the_same_panel() {
        let (screen, mut picker) = setup("ui", "consul");
        open(&mut picker);
        let mut t = terminal(120, 30);
        render(&mut t, &screen, &mut picker, &ready());
        assert_snapshot!("help_120x30", shown(&t));
    }
}
