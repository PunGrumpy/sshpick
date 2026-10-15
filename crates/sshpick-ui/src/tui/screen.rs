use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use sshpick_core::{Entry, Picker, Warning};

use super::bar::{Bar, RULE, bar_style, rule};
use super::pane::{self, Pane};
use super::popup::popup;
use super::size::{SIDE_PANE_WIDTH, STACKED_ROWS, Tier, tier};
use super::table::{HostTable, prompt_row};

#[derive(Debug)]
pub struct Screen {
    table: HostTable,
    bar: Bar,
    warnings: Vec<String>,
}

impl Screen {
    pub fn new(entries: &[Entry], warnings: &[Warning]) -> Screen {
        let bar = bar_style(&|name| std::env::var_os(name));
        Screen::styled(entries, warnings, bar)
    }

    pub(super) fn styled(entries: &[Entry], warnings: &[Warning], bar: Style) -> Screen {
        Screen {
            table: HostTable::new(entries),
            bar: Bar::styled(warnings, bar),
            warnings: warnings.iter().map(ToString::to_string).collect(),
        }
    }

    pub fn draw(&self, frame: &mut Frame, picker: &mut Picker, pane: Pane<'_>) {
        let area = frame.area();
        let tier = tier(area);
        let framed = tier.framed();
        let side = tier == Tier::Side;
        let stacked = tier == Tier::Stacked;
        let [content, status] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let [prompt, top_rule, header, list, bottom_rule, stacked_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(u16::from(framed)),
            Constraint::Length(u16::from(framed)),
            Constraint::Fill(1),
            Constraint::Length(u16::from(framed)),
            Constraint::Length(if stacked { STACKED_ROWS } else { 0 }),
        ])
        .areas(content);
        let split = |area| {
            let [left, right] = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Length(if side { SIDE_PANE_WIDTH } else { 0 }),
            ])
            .areas(area);
            (left, right)
        };
        let (header, header_side) = split(header);
        let (list, list_side) = split(list);

        let view = picker.view(list.height.into());
        prompt_row(frame, prompt, &view);
        self.table.list(frame, list, &view, self.bar.style());
        if framed {
            self.table.header(frame, header);
            frame.render_widget(rule(top_rule.width, "", ""), top_rule);
            let stacked_title = pane
                .filter(|_| stacked)
                .map(|(host, slot)| format!("{RULE} {} ", pane::title(host, slot)));
            let more = (view.below > 0).then(|| format!(" {} more below {RULE}", view.below));
            let line = rule(
                bottom_rule.width,
                stacked_title.as_deref().unwrap_or_default(),
                more.as_deref().unwrap_or_default(),
            );
            frame.render_widget(line, bottom_rule);
        }
        if side {
            pane::side(frame, header_side.union(list_side), pane);
        }
        if stacked {
            pane::stacked(frame, stacked_area, pane);
        }
        self.bar.status(frame, status, view.total);
        if picker.help_open() {
            popup(frame, &self.warnings);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use insta::assert_snapshot;
    use ratatui::style::{Color, Modifier};
    use sshpick_core::{Key, Preview, ResolveError, Slot};

    use super::*;
    use crate::tui::testing::{
        DARK, LIGHT, cell, frame, inverted, ready, render, setup, setup_with, terminal, vars,
    };

    fn beside(left: &str, right: &str) -> String {
        format!("{left:<60} │ {right}").trim_end().to_owned()
    }

    fn bar_line(width: usize) -> String {
        format!(
            " Enter ssh  Esc quit  Ctrl-U clear  ↑↓ move{}5 hosts",
            " ".repeat(width - 2 - 42 - 7)
        )
    }

    fn rule_line(width: usize) -> String {
        "─".repeat(width)
    }

    fn prompt_line(left: &str, count: &str, width: usize) -> String {
        format!("{left:<w$}{count}", w = width - count.len())
    }

    #[test]
    fn rows_list_top_down_under_the_header_and_the_count_sits_on_the_prompt() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("list_60x20", frame(&lines));
        assert!((0..60).all(|x| inverted(&cell(&terminal, x, 3), DARK)));
        assert!((0..60).all(|x| !inverted(&cell(&terminal, x, 4), DARK)));
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn env_column_hides_when_no_host_has_a_label() {
        let (screen, mut picker) = setup("flat", "");
        let lines = render(&mut terminal(40, 14), &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("flat_40x14", frame(&lines));
    }

    #[test]
    fn narrowing_the_query_leaves_no_stale_rows() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let all = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(all[3..8].iter().filter(|line| !line.is_empty()).count(), 5);

        for c in "'lab-b".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("narrowed_60x20", frame(&lines));
    }

    #[test]
    fn the_pane_never_shows_another_hosts_lines() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let failed = Slot::Failed(ResolveError::Timeout);
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert_snapshot!("failed_60x20", frame(&lines));

        picker.handle(Key::Down);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("waiting_after_down_60x20", frame(&lines));

        for c in "zzzz".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("no_match_60x20", frame(&lines));
    }

    #[test]
    fn a_failed_preview_wraps_and_stops_at_five_rows() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(40, 20);
        let stderr = "word ".repeat(60);
        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr,
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert_snapshot!("failed_wraps_40x20", frame(&lines));
    }

    #[test]
    fn matched_characters_are_bold_not_coloured_and_the_cursor_follows_the_query() {
        let (screen, mut picker) = setup("ui", "'lab-b");
        let mut terminal = terminal(60, 20);
        render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let host = cell(&terminal, 14, 3);
        assert_eq!(host.symbol(), "l");
        assert!(host.modifier.contains(Modifier::BOLD));
        assert!(inverted(&host, DARK), "bold survives on the selected bar");
        let env = cell(&terminal, 2, 3);
        assert_eq!(env.symbol(), "L");
        assert!(!env.modifier.contains(Modifier::BOLD), "LAB is not matched");
        terminal.backend_mut().assert_cursor_position((2 + 6, 0));

        let buffer = terminal.backend().buffer();
        assert!(
            buffer.content().iter().all(|cell| cell.fg != Color::Green),
            "no hue is used for matches"
        );
    }

    #[test]
    fn dark_100x16_consul_snapshot_has_the_pane_beside_the_list() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(100, 16);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("side_100x16_consul", frame(&lines));

        assert!((0..60).all(|x| inverted(&cell(&terminal, x, 3), DARK)));
        assert!((0..60).all(|x| !cell(&terminal, x, 3).modifier.contains(Modifier::DIM)));
        assert!((60..100).all(|x| !inverted(&cell(&terminal, x, 3), DARK)));
        assert!((0..100).all(|x| !inverted(&cell(&terminal, x, 4), DARK)));
        for y in 2..14 {
            assert_eq!(cell(&terminal, 61, y).symbol(), "│", "{y}");
            assert!(cell(&terminal, 61, y).modifier.contains(Modifier::DIM));
        }
        assert_eq!(cell(&terminal, 61, 1).symbol(), "─", "the rule is not cut");
        assert_eq!(cell(&terminal, 61, 14).symbol(), "─");

        let env = cell(&terminal, 2, 4);
        assert!(env.modifier.contains(Modifier::DIM), "ENV is context");
        let host = cell(&terminal, 14, 4);
        assert!(!host.modifier.contains(Modifier::DIM), "HOST is the thing");
        let matched = cell(&terminal, 14 + 7, 4);
        assert_eq!(matched.symbol(), "c");
        assert!(matched.modifier.contains(Modifier::BOLD));
        assert!(!cell(&terminal, 14 + 6, 4).modifier.contains(Modifier::BOLD));
        assert!(
            cell(&terminal, 2, 2).modifier.contains(Modifier::DIM),
            "header"
        );
        let title = cell(&terminal, 63, 2);
        assert_eq!(title.symbol(), "s");
        assert!(title.modifier.contains(Modifier::DIM), "pane title");
        let key = cell(&terminal, 63, 3);
        assert_eq!(key.symbol(), "u");
        assert!(key.modifier.contains(Modifier::DIM), "pane key");
        let value = cell(&terminal, 63 + 14, 3);
        assert_eq!(value.symbol(), "u");
        assert!(!value.modifier.contains(Modifier::DIM), "pane value");

        assert!(
            cell(&terminal, 0, 0).modifier.contains(Modifier::BOLD),
            "prompt"
        );
        assert!(!cell(&terminal, 2, 0).modifier.contains(Modifier::BOLD));
        let count = cell(&terminal, 97, 0);
        assert_eq!(count.symbol(), "2");
        assert!(count.modifier.contains(Modifier::DIM), "count");
        assert!(
            cell(&terminal, 50, 1).modifier.contains(Modifier::DIM),
            "top rule"
        );
        assert!(
            cell(&terminal, 50, 14).modifier.contains(Modifier::DIM),
            "bottom rule"
        );
        terminal.backend_mut().assert_cursor_position((2 + 6, 0));

        assert!((0..100).all(|x| inverted(&cell(&terminal, x, 15), DARK)));
        assert!(
            cell(&terminal, 1, 15).modifier.contains(Modifier::BOLD),
            "Enter"
        );
        assert!(
            !cell(&terminal, 7, 15).modifier.contains(Modifier::BOLD),
            "ssh"
        );
        assert!(!cell(&terminal, 99, 15).modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn dark_80x24_consul_snapshot_stacks_the_preview_under_the_list() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("stacked_80x24_consul", frame(&lines));

        assert!((0..80).all(|x| inverted(&cell(&terminal, x, 3), DARK)));
        assert!((0..80).all(|x| !inverted(&cell(&terminal, x, 4), DARK)));
        assert!(
            cell(&terminal, 5, 17).modifier.contains(Modifier::DIM),
            "title"
        );
        assert!(
            cell(&terminal, 70, 17).modifier.contains(Modifier::DIM),
            "rule"
        );
        assert!(
            cell(&terminal, 2, 18).modifier.contains(Modifier::DIM),
            "key"
        );
        assert!(
            !cell(&terminal, 16, 18).modifier.contains(Modifier::DIM),
            "value"
        );
        terminal.backend_mut().assert_cursor_position((2 + 6, 0));
    }

    #[test]
    fn small_60x14_terminal_draws_what_fits() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 14);
        let failed = Slot::Failed(ResolveError::Timeout);
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert_snapshot!("small_60x14_failed", frame(&lines));
    }

    #[test]
    fn no_color_draws_with_modifiers_only() {
        let bar = bar_style(&vars(&[("NO_COLOR", "1"), ("SSHPICK_THEME", "light")]));
        let (screen, mut picker) = setup_with("ui", "consul", bar, &[]);
        let mut terminal = terminal(120, 40);
        render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let buffer = terminal.backend().buffer();
        assert!(
            buffer
                .content()
                .iter()
                .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset),
            "no colour of any kind"
        );
        assert!((0..80).all(|x| cell(&terminal, x, 3).modifier.contains(Modifier::REVERSED)));
        assert!((0..120).all(|x| cell(&terminal, x, 39).modifier.contains(Modifier::REVERSED)));
        assert!((0..120).all(|x| !cell(&terminal, x, 2).modifier.contains(Modifier::REVERSED)));
        assert!((0..120).all(|x| !cell(&terminal, x, 4).modifier.contains(Modifier::REVERSED)));
        assert!(cell(&terminal, 1, 39).modifier.contains(Modifier::BOLD));
        assert!(!cell(&terminal, 7, 39).modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn light_theme_swaps_the_inverted_colours() {
        let bar = bar_style(&vars(&[("SSHPICK_THEME", "light")]));
        assert_eq!(bar, LIGHT);
        let (screen, mut picker) = setup_with("ui", "consul", bar, &[]);
        let mut terminal = terminal(120, 40);
        render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!((0..80).all(|x| inverted(&cell(&terminal, x, 3), LIGHT)));
        assert!((0..120).all(|x| inverted(&cell(&terminal, x, 39), LIGHT)));
        assert!(!inverted(&cell(&terminal, 12, 4), LIGHT));
    }

    #[test]
    fn content_spans_the_whole_terminal_at_any_width() {
        let (screen, mut picker) = setup("ui", "");
        for width in [60, 99, 100, 101, 160, 200] {
            let wide = width >= 100;
            let list_width = if wide { width - 40 } else { width };
            let mut terminal = terminal(width, 20);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            assert!(lines[0].starts_with("> type to filter"), "{width}");
            assert!(lines[0].ends_with("5/5"), "{width}");
            assert_eq!(lines[0].chars().count(), usize::from(width), "{width}");
            assert_eq!(lines[1], rule_line(width.into()), "{width}");
            assert!(lines[3].starts_with("▌ LAB         lab-a"), "{width}");
            assert!((0..list_width).all(|x| inverted(&cell(&terminal, x, 3), DARK)));
            assert!((list_width..width).all(|x| !inverted(&cell(&terminal, x, 3), DARK)));
            assert!((0..width).all(|x| inverted(&cell(&terminal, x, 19), DARK)));
            let bottom = if wide { 18 } else { 13 };
            assert!(
                lines[bottom].chars().all(|c| c != ' ') || lines[bottom].starts_with("─ ssh"),
                "{width}: {:?}",
                lines[bottom]
            );
            assert_eq!(lines[bottom].chars().count(), usize::from(width), "{width}");
            assert_eq!(lines[2].contains('│'), wide, "{width}");
            terminal.backend_mut().assert_cursor_position((2, 0));
        }
    }

    #[test]
    fn a_long_what_ends_in_an_ellipsis_on_a_character_boundary() {
        let (screen, mut picker) = setup("ui", "consul");
        let row = "▌ NEW STAGING newstg-consul1 ";
        for (width, what) in [
            (46, "consul server 1 …"),
            (47, "consul server 1 —…"),
            (48, "consul server 1 — …"),
            (49, "consul server 1 — m…"),
            (50, "consul server 1 — mg…"),
            (51, "consul server 1 — mgmt"),
            (80, "consul server 1 — mgmt"),
        ] {
            let mut terminal = terminal(width, 20);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            assert_eq!(lines[3], format!("{row}{what}"), "{width}");
            assert_eq!(lines[4], "  NEW STAGING newstg-consul2", "{width}");
        }
    }

    #[test]
    fn bold_survives_inside_the_kept_part_and_the_ellipsis_is_plain() {
        let (screen, mut picker) = setup("ui", "'server 1");
        let mut terminal = terminal(41, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[3], "▌ NEW STAGING newstg-consul1 consul serv…");
        let bold = |x| cell(&terminal, x, 3).modifier.contains(Modifier::BOLD);
        assert!(!(29..36).any(bold), "consul ");
        assert!((36..40).all(bold), "serv");
        assert_eq!(cell(&terminal, 40, 3).symbol(), "…");
        assert!(!bold(40), "the ellipsis is not a match");
        assert!(inverted(&cell(&terminal, 40, 3), DARK));
    }

    #[test]
    fn the_prompt_shows_a_dim_placeholder_until_the_first_key() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[0], prompt_line("> type to filter", "5/5", 60));
        assert!(cell(&terminal, 0, 0).modifier.contains(Modifier::BOLD));
        assert!((2..16).all(|x| cell(&terminal, x, 0).modifier.contains(Modifier::DIM)));
        assert!((2..16).all(|x| !cell(&terminal, x, 0).modifier.contains(Modifier::BOLD)));
        terminal.backend_mut().assert_cursor_position((2, 0));

        picker.handle(Key::Char('l'));
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[0], prompt_line("> l", "4/5", 60));
        assert!(!cell(&terminal, 2, 0).modifier.contains(Modifier::DIM));
        terminal.backend_mut().assert_cursor_position((3, 0));

        picker.handle(Key::Ctrl('u'));
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[0],
            prompt_line("> type to filter", "5/5", 60),
            "clearing brings it back"
        );
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn the_count_stays_on_the_right_when_the_query_outgrows_the_room() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(30, 20);
        for c in "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let room = 30 - 2 - 4;
        assert_eq!(
            lines[0],
            format!("> {}  0/5", "z".repeat(room - 1)),
            "the query scrolls, the cursor cell, a space, the count"
        );
        terminal
            .backend_mut()
            .assert_cursor_position((2 + room as u16 - 1, 0));
        assert!(cell(&terminal, 27, 0).modifier.contains(Modifier::DIM));
        assert!(!cell(&terminal, 24, 0).modifier.contains(Modifier::DIM));
        assert_eq!(cell(&terminal, 25, 0).symbol(), " ", "the cursor cell");
    }

    #[test]
    fn zero_matches_say_no_match_on_the_first_row_and_the_bar_keeps_the_count() {
        let warnings = [Warning::Cycle {
            path: PathBuf::from("/x"),
        }];
        let (screen, mut picker) = setup_with("ui", "zzzz", DARK, &warnings);
        let mut wide = terminal(120, 20);
        let lines = render(&mut wide, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("no_match_120x20", frame(&lines));
        assert!(
            (2..10).all(|x| cell(&wide, x, 3).modifier.contains(Modifier::DIM)),
            "no match is dim"
        );
        assert!((0..120).all(|x| !inverted(&cell(&wide, x, 3), DARK)));
        assert!((0..120).all(|x| inverted(&cell(&wide, x, 19), DARK)));
        assert!(
            (0..120).all(|x| !cell(&wide, x, 19).modifier.contains(Modifier::DIM)),
            "the bar has no dim"
        );

        let (screen, mut picker) = setup("ui", "zzzz");
        let mut small = terminal(60, 20);
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[3], "  no match");
        assert_eq!(
            lines[19],
            bar_line(60),
            "the bar says 5 hosts, not no match"
        );
    }

    #[test]
    fn hidden_rows_are_counted_on_the_bottom_rule() {
        let (screen, mut picker) = setup("ui", "");
        let mut count = |height: u16| {
            let mut terminal = terminal(60, height);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            let preview = if height >= 13 { 5 } else { 0 };
            let list = height - 5 - preview;
            lines[usize::from(3 + list)].clone()
        };
        let more = |line: &str| line.ends_with("more below ─");
        assert!(!more(&count(16)), "all five rows fit");
        assert!(!more(&count(15)), "exactly fit");
        assert!(count(14).ends_with(" 1 more below ─"));
        assert!(count(13).ends_with(" 2 more below ─"));
        assert!(!more(&count(12)), "the list takes the preview's room");
        assert!(!more(&count(10)), "exactly fit without a preview");
        assert!(count(9).ends_with(" 1 more below ─"));
        assert!(count(8).ends_with(" 2 more below ─"));
        assert!(count(7).ends_with(" 3 more below ─"));
        assert_eq!(count(9), format!("{} 1 more below ─", "─".repeat(60 - 15)));

        let mut scrolled = terminal(60, 13);
        for _ in 0..4 {
            picker.handle(Key::Down);
        }
        let lines = render(&mut scrolled, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[3],
            "  NEW STAGING newstg-consul1 consul server 1 — mgmt"
        );
        assert_eq!(lines[5], "▌ NEW STAGING newstg-bastion bastion");
        assert!(
            lines[6].starts_with("─ ssh -G newstg-bastion ─") && !more(&lines[6]),
            "scrolled to the end, nothing below: {:?}",
            lines[6]
        );

        let (screen, mut picker) = setup("ui", "'lab");
        let mut small = terminal(60, 13);
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[3], "▌ LAB         lab-b          build box");
        assert_eq!(lines[4], "  LAB         lab-a          LAB — personal VMs");
        assert_eq!(lines[0], prompt_line("> 'lab", "2/5", 60));
        assert!(!more(&lines[6]), "two matches, three rows, none hidden");
    }

    #[test]
    fn the_wide_pane_shows_one_more_below_on_a_short_list() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 9);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("side_100x9_one_below", frame(&lines));
        assert!(cell(&terminal, 90, 7).modifier.contains(Modifier::DIM));
        assert!(cell(&terminal, 80, 7).modifier.contains(Modifier::DIM));
    }

    #[test]
    fn the_wide_pane_cuts_long_values_and_wraps_errors_inside_its_37_columns() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 16);
        let long = Slot::Ready(Preview::parse(
            "user ubuntu\nproxyjump a-very-long-bastion-host-name\n",
        ));
        let lines = render(&mut terminal, &screen, &mut picker, &long);
        assert!(
            lines[4].ends_with("proxyjump     a-very-long-bastion-ho…"),
            "{:?}",
            lines[4]
        );
        assert_eq!(lines[4].chars().count(), 100);
        assert_eq!(cell(&terminal, 99, 4).symbol(), "…");

        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "word ".repeat(30),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert!(
            lines[2].ends_with("│ ssh -G lab-a failed"),
            "{:?}",
            lines[2]
        );
        for line in &lines[3..8] {
            let pane: String = line.chars().skip(63).collect();
            assert!(
                pane.starts_with("word") && pane.chars().count() <= 37,
                "{pane:?}"
            );
        }
        assert!(lines[8].ends_with("│"), "{:?}", lines[8]);
    }

    #[test]
    fn a_waiting_pane_shows_only_its_title_and_no_selection_shows_only_the_divider() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(100, 16);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!(lines[2].ends_with("│ ssh -G newstg-consul1"));
        assert!(lines[3..14].iter().all(|line| line.ends_with('│')));

        for c in "zzzz".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[2], beside("  ENV         HOST           WHAT", ""));
        assert!(lines[3..14].iter().all(|line| line.ends_with('│')));
        assert!(
            !lines.iter().any(|line| line.contains("ssh -G")),
            "{lines:#?}"
        );
    }

    #[test]
    fn a_100x12_terminal_still_shows_the_side_pane_and_the_list_takes_the_rest() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("side_100x12", frame(&lines));
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn an_80x12_terminal_has_no_preview_and_the_list_takes_its_room() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("list_only_80x12", frame(&lines));
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn a_100x6_terminal_keeps_only_the_prompt_the_list_and_the_status_bar() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 6);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("bare_100x6", frame(&lines));
        assert!((0..100).all(|x| inverted(&cell(&terminal, x, 1), DARK)));
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn the_stacked_preview_needs_13_rows_and_the_frame_needs_7() {
        let (screen, mut picker) = setup("ui", "");
        let mut lines_at = |width, height| {
            let mut terminal = terminal(width, height);
            render(&mut terminal, &screen, &mut picker, &Slot::Waiting)
        };
        let has =
            |lines: &[String], wanted: &dyn Fn(&str) -> bool| lines.iter().any(|line| wanted(line));
        let title = |line: &str| line.starts_with("─ ssh -G");
        let header = |line: &str| line.starts_with("  ENV         HOST           WHAT");
        let rule = |line: &str| line.starts_with('─');

        assert!(has(&lines_at(80, 13), &title));
        assert!(!has(&lines_at(80, 12), &title));
        assert!(
            !has(&lines_at(100, 40), &title),
            "wide has no stacked preview"
        );
        for width in [80, 100] {
            for (height, shown) in [(7, true), (6, false)] {
                let lines = lines_at(width, height);
                assert_eq!(has(&lines, &header), shown, "{width}x{height}");
                assert_eq!(
                    lines.iter().filter(|line| rule(line)).count(),
                    if shown { 2 } else { 0 },
                    "{width}x{height}: {lines:#?}"
                );
            }
        }
        let lines = lines_at(80, 7);
        assert!(lines[0].starts_with("> "));
        assert!(lines[3].starts_with("▌ LAB"));
        assert!(lines[4].starts_with("  LAB         lab-b"));
        assert_eq!(lines[5], format!("{} 3 more below ─", "─".repeat(80 - 15)));
        assert_eq!(lines[6], bar_line(80));
    }

    #[test]
    fn tiny_terminals_never_panic() {
        let (screen, mut picker) = setup("ui", "consul");
        let sizes = (0..=45)
            .map(|width| (width, 14))
            .chain([(99, 20), (100, 20), (101, 20), (140, 20)])
            .flat_map(|(width, most)| (0..=most).map(move |height| (width, height)));
        for (width, height) in sizes {
            let mut terminal = terminal(width, height);
            let failed = Slot::Failed(ResolveError::Timeout);
            render(&mut terminal, &screen, &mut picker, &failed);
            let lines = render(&mut terminal, &screen, &mut picker, &ready());
            if height >= 2 && width >= 1 {
                let prompt = &lines[0];
                assert!(prompt.starts_with('>'), "{width}x{height}: {lines:#?}");
            }
        }
        let lines = render(&mut terminal(30, 1), &screen, &mut picker, &Slot::Waiting);
        assert!(lines[0].starts_with(" Enter ssh"), "the bar still draws");
    }

    #[test]
    fn the_status_counts_one_host_and_one_warning_in_the_singular() {
        let warnings = [Warning::Cycle {
            path: PathBuf::from("/x"),
        }];
        let (screen, mut picker) = setup_with("home", "", DARK, &warnings);
        let lines = render(&mut terminal(100, 12), &screen, &mut picker, &Slot::Waiting);
        assert!(
            lines[11].ends_with("1 host · 1 warning · /x: skipped, include cycle"),
            "{:?}",
            lines[11]
        );
    }
}
