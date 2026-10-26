use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use sshpick_core::{Entry, Picker, Slot};

use super::help::{self, help};
use super::plan::{Plan, Space, plan};
use super::route;
use super::table::{HostTable, hint_row, prompt_row};

#[derive(Debug)]
pub struct Screen {
    table: HostTable,
    hosts: usize,
}

impl Screen {
    pub fn new(entries: &[Entry]) -> Screen {
        Screen {
            table: HostTable::new(entries),
            hosts: entries.len(),
        }
    }

    pub(super) fn plan(&self, space: Space, picker: &Picker) -> Plan {
        let help = if picker.help_open() { help::rows() } else { 0 };
        plan(space, self.hosts, help)
    }

    /// `frame.area()` is the block `plan` describes.
    pub(super) fn draw(
        &self,
        frame: &mut Frame,
        picker: &mut Picker,
        slot: Option<&Slot>,
        plan: Plan,
    ) {
        let [prompt, beside, route_area, help_area, hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(plan.beside()),
            Constraint::Length(u16::from(plan.route)),
            Constraint::Length(plan.help),
            Constraint::Length(u16::from(plan.hint)),
        ])
        .areas(frame.area());
        let [left, panel] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(plan.panel_cols())])
                .areas(beside);
        let [header, list] = Layout::vertical([
            Constraint::Length(u16::from(plan.header)),
            Constraint::Length(plan.list),
        ])
        .areas(left);

        picker.follow(list.height.into());
        let view = picker.view(list.height.into());
        // With no row for the list, the prompt row names the selection so Enter is never blind.
        let selection = picker
            .selected()
            .filter(|_| plan.list == 0)
            .map(|host| host.as_str());
        prompt_row(frame, prompt, &view, selection);
        self.table.list(frame, list, &view);
        self.table.header(frame, header);
        route::row(frame, route_area, slot);
        if plan.panel {
            let selected = picker.selected().zip(slot);
            route::panel(frame, panel, selected);
        }
        help(frame, help_area);
        hint_row(frame, hint_area, &view);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use insta::assert_snapshot;
    use ratatui::style::Color;
    use sshpick_core::{Key, Preview, ResolveError, Slot};
    use test_case::test_case;

    use crate::tui::testing::{
        bold, cell, dim, frame, ready, render, reversed, setup, styles, terminal,
    };

    const KEYS: &str = "  ↑↓ move   enter ssh   esc quit   ? keys";

    #[test]
    fn the_reference_screens_are_drawn_cell_for_cell() {
        let (screen, mut picker) = setup("ui", "");
        let mut empty = terminal(80, 24);
        let lines = render(&mut empty, &screen, &mut picker, &ready());
        assert_snapshot!("empty_80x24", frame(&lines));
        assert_snapshot!("empty_80x24_styles", styles(&empty));
        empty.backend_mut().assert_cursor_position((2, 0));

        let (screen, mut picker) = setup("ui", "consul");
        let mut consul = terminal(80, 24);
        let lines = render(&mut consul, &screen, &mut picker, &ready());
        assert_snapshot!("consul_80x24", frame(&lines));
        assert_snapshot!("consul_80x24_styles", styles(&consul));
        consul.backend_mut().assert_cursor_position((2 + 6, 0));
    }

    fn sample<'a>(text: &'a str, command: &str) -> Option<Vec<&'a str>> {
        text.split("```text\n")
            .find_map(|block| block.strip_prefix(command)?.strip_prefix('\n'))
            .and_then(|block| block.split("\n```").next())
            .map(|block| block.lines().collect())
    }

    #[test]
    fn the_docs_show_the_real_screen() {
        let (screen, mut picker) = setup("ui", "");
        let lines = render(&mut terminal(80, 12), &screen, &mut picker, &ready());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |doc| std::fs::read_to_string(root.join(doc)).unwrap();
        for doc in ["README.md", "docs/picker.md"] {
            let text = read(doc);
            let shown = sample(&text, "~/work $ sshpick");
            assert_eq!(
                shown,
                Some(lines.iter().map(String::as_str).collect()),
                "{doc}"
            );
        }

        let (screen, mut picker) = setup("ui", "consul");
        let wide = render(&mut terminal(100, 12), &screen, &mut picker, &ready());
        let text = read("docs/picker.md");
        let shown = sample(&text, "~/work $ sshpick consul");
        assert_eq!(
            shown,
            Some(wide.iter().map(String::as_str).collect()),
            "the panel"
        );
    }

    #[test]
    fn rows_list_top_down_under_the_header_and_the_count_sits_on_the_prompt() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("waiting_60x20", frame(&lines));
        assert!((0..60).all(|x| reversed(&cell(&terminal, x, 2))));
        assert!((0..60).all(|x| !reversed(&cell(&terminal, x, 3))));
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
        assert_eq!(all[2..7].iter().filter(|line| !line.is_empty()).count(), 5);

        for c in "'lab-b".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("narrowed_60x20", frame(&lines));
    }

    #[test]
    fn the_hint_row_stays_on_the_last_row_while_the_query_changes() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        for query in ["", "c", "consul", "consul1", "zzzz"] {
            picker.handle(Key::Ctrl('u'));
            for c in query.chars() {
                picker.handle(Key::Char(c));
            }
            let lines = render(&mut terminal, &screen, &mut picker, &ready());
            assert_eq!(lines.len(), 24, "{query:?}: {lines:#?}");
            assert!(lines[23].contains("esc quit"), "{query:?}: {lines:#?}");
        }
    }

    #[test]
    fn the_route_never_shows_another_hosts_lines() {
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
    fn a_failed_route_is_one_row_cut_with_an_ellipsis() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(40, 9);
        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: format!("{}\nsecond line", "word ".repeat(60)),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert_eq!(lines[7], "  → ssh -G failed: word word word word …");
        assert!(!lines.concat().contains("second line"));
        assert_eq!(
            lines[6], "  NEW STAGING newstg-bastion bastion",
            "nothing wraps onto the rows above"
        );
        assert!(dim(&cell(&terminal, 2, 7)) && !dim(&cell(&terminal, 4, 7)));
    }

    #[test]
    fn matched_characters_are_bold_and_the_cursor_follows_the_query() {
        let (screen, mut picker) = setup("ui", "'lab-b");
        let mut terminal = terminal(60, 20);
        render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let host = cell(&terminal, 14, 2);
        assert_eq!(host.symbol(), "l");
        assert!(
            bold(&host) && reversed(&host),
            "bold survives on the selected row"
        );
        let env = cell(&terminal, 2, 2);
        assert_eq!(env.symbol(), "L");
        assert!(!bold(&env), "LAB is not matched");
        terminal.backend_mut().assert_cursor_position((2 + 6, 0));
    }

    #[test]
    fn the_screen_uses_no_colour_so_there_is_none_to_turn_off() {
        for (query, width, height) in [("", 120, 30), ("consul", 80, 24), ("zzzz", 100, 12)] {
            let (screen, mut picker) = setup("ui", query);
            picker.handle(Key::F(1));
            let mut terminal = terminal(width, height);
            render(&mut terminal, &screen, &mut picker, &ready());
            let buffer = terminal.backend().buffer();
            assert!(
                buffer
                    .content()
                    .iter()
                    .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset),
                "{query:?} at {width}x{height}"
            );
        }
    }

    #[test]
    fn the_selection_is_reversed_with_no_dim_and_the_rest_keeps_its_context() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!((2..13).all(|x| reversed(&cell(&terminal, x, 2)) && !dim(&cell(&terminal, x, 2))));
        assert!(
            dim(&cell(&terminal, 2, 3)),
            "ENV on an unselected row is dim"
        );
        assert!(!dim(&cell(&terminal, 14, 3)), "HOST is the thing");
        assert!(dim(&cell(&terminal, 30, 3)), "WHAT is context");
        assert!(dim(&cell(&terminal, 2, 1)), "the header is dim");
        assert!(bold(&cell(&terminal, 0, 0)), "the prompt marker");
    }

    #[test]
    fn the_only_rule_is_the_panels_from_100_columns() {
        for (width, height) in [(80, 24), (80, 12), (99, 20), (100, 24), (120, 30), (60, 6)] {
            let (screen, mut picker) = setup("ui", "");
            let lines = render(&mut terminal(width, height), &screen, &mut picker, &ready());
            let drawn: String = lines.concat();
            assert!(
                !drawn.contains(['─', '┌', '┐', '└', '┘']),
                "{width}x{height}: {lines:#?}"
            );
            let rules = drawn.matches('│').count();
            let want = if width < 100 {
                0
            } else {
                usize::from(height) - 2
            };
            assert_eq!(rules, want, "{width}x{height}: {lines:#?}");
        }
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
    fn content_spans_the_whole_terminal_at_any_width() {
        let (screen, mut picker) = setup("ui", "");
        for width in [60, 99] {
            let mut terminal = terminal(width, 9);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            assert_eq!(lines.len(), 9, "{width}: one layout under 100 columns");
            assert!(lines[0].starts_with("> type to filter"), "{width}");
            assert!(lines[0].ends_with("5/5"), "{width}");
            assert_eq!(lines[0].chars().count(), usize::from(width));
            assert!(lines[2].starts_with("▌ LAB         lab-a"), "{width}");
            assert_eq!(lines[7], "  → resolving…", "{width}");
            assert_eq!(lines[8], KEYS, "{width}");
            assert!((0..width).all(|x| reversed(&cell(&terminal, x, 2))));
            terminal.backend_mut().assert_cursor_position((2, 0));
        }
        for width in [100, 101, 160, 200] {
            let mut terminal = terminal(width, 8);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            let rule = width - width / 5 * 2;
            assert_eq!(lines.len(), 8, "{width}: the route needs no row");
            assert_eq!(lines[0].chars().count(), usize::from(width), "{width}");
            assert!(lines[0].ends_with("5/5"), "{width}");
            assert!(lines[2].starts_with("▌ LAB         lab-a"), "{width}");
            assert!((0..rule).all(|x| reversed(&cell(&terminal, x, 2))));
            assert!(
                !reversed(&cell(&terminal, rule, 2)),
                "the panel is not selected"
            );
            assert_eq!(lines[7], KEYS, "{width}");
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
            assert_eq!(lines[2], format!("{row}{what}"), "{width}");
            assert_eq!(lines[3], "  NEW STAGING newstg-consul2", "{width}");
        }
    }

    #[test]
    fn a_what_too_narrow_for_more_than_its_ellipsis_is_left_out() {
        let (screen, mut picker) = setup("ui", "");
        for (width, row) in [
            (30, "▌ LAB         lab-a"),
            (31, "▌ LAB         lab-a"),
            (32, "▌ LAB         lab-a          LA…"),
            (33, "▌ LAB         lab-a          LAB…"),
        ] {
            let lines = render(&mut terminal(width, 3), &screen, &mut picker, &ready());
            assert_eq!(lines[1].trim_end(), row, "{width}");
        }
    }

    #[test]
    fn bold_survives_inside_the_kept_part_and_the_ellipsis_is_plain() {
        let (screen, mut picker) = setup("ui", "'server 1");
        let mut terminal = terminal(41, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[2], "▌ NEW STAGING newstg-consul1 consul serv…");
        let is_bold = |x| bold(&cell(&terminal, x, 2));
        assert!(!(29..36).any(is_bold), "consul ");
        assert!((36..40).all(is_bold), "serv");
        assert_eq!(cell(&terminal, 40, 2).symbol(), "…");
        assert!(!is_bold(40), "the ellipsis is not a match");
        assert!(reversed(&cell(&terminal, 40, 2)));
    }

    #[test]
    fn the_prompt_shows_a_dim_placeholder_until_the_first_key() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[0], format!("{:<57}5/5", "> type to filter"));
        assert!(bold(&cell(&terminal, 0, 0)));
        assert!((2..16).all(|x| dim(&cell(&terminal, x, 0)) && !bold(&cell(&terminal, x, 0))));
        terminal.backend_mut().assert_cursor_position((2, 0));

        picker.handle(Key::Char('l'));
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[0], format!("{:<57}4/5", "> l"));
        assert!(!dim(&cell(&terminal, 2, 0)));
        terminal.backend_mut().assert_cursor_position((3, 0));

        picker.handle(Key::Ctrl('u'));
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[0],
            format!("{:<57}5/5", "> type to filter"),
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
        assert!(dim(&cell(&terminal, 27, 0)));
        assert!(!dim(&cell(&terminal, 24, 0)));
        assert_eq!(cell(&terminal, 25, 0).symbol(), " ", "the cursor cell");
    }

    #[test]
    fn zero_matches_say_no_matching_hosts_on_the_first_row_and_the_hint_offers_only_quit() {
        let (screen, mut picker) = setup("ui", "zzzz");
        let mut wide = terminal(99, 9);
        let lines = render(&mut wide, &screen, &mut picker, &ready());
        assert_snapshot!("no_match_99x9", frame(&lines));
        assert_eq!(lines[2], "  no matching hosts");
        assert!((0..19).all(|x| dim(&cell(&wide, x, 2))), "no match is dim");
        assert!((19..99).all(|x| !dim(&cell(&wide, x, 2))));
        assert!((0..99).all(|x| !reversed(&cell(&wide, x, 2))));
        assert!(lines[7].is_empty(), "there is no route to show");
        assert_eq!(lines[8], "  esc quit   F1 keys", "no enter, no move");

        let lines = render(&mut terminal(120, 8), &screen, &mut picker, &ready());
        let panel: Vec<&str> = lines[1..7]
            .iter()
            .map(|line| &line[line.len() - 3..])
            .collect();
        assert_eq!(panel, ["│"; 6], "the panel has no host to show: {lines:#?}");
        assert_eq!(lines[7], "  esc quit   F1 keys");
    }

    #[test]
    fn the_window_range_sits_flush_right_on_the_hint_row_when_the_list_scrolls() {
        let (screen, mut picker) = setup("ui", "");
        let mut last = |rows: u16| {
            let mut terminal = terminal(60, rows);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            lines.last().cloned().unwrap()
        };
        for rows in [24, 18, 12, 9] {
            assert_eq!(last(rows), KEYS, "{rows}: all five fit");
        }
        assert_eq!(last(8), format!("{KEYS:<57}1–4"));
        assert_eq!(last(7), format!("{KEYS:<57}1–3"));
        assert_eq!(last(6), format!("{KEYS:<57}1–3"), "the header went first");

        let mut scrolled = terminal(60, 7);
        for _ in 0..4 {
            picker.handle(Key::Down);
        }
        let lines = render(&mut scrolled, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[2],
            "  NEW STAGING newstg-consul1 consul server 1 — mgmt"
        );
        assert_eq!(lines[4], "▌ NEW STAGING newstg-bastion bastion");
        assert_eq!(lines[6], format!("{KEYS:<57}3–5"), "scrolled to the end");

        let (screen, mut picker) = setup("ui", "'lab");
        let lines = render(&mut terminal(60, 7), &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[2], "▌ LAB         lab-b          build box");
        assert_eq!(lines[3], "  LAB         lab-a          LAB — personal VMs");
        assert!(lines[4].is_empty(), "the block keeps its height");
        assert_eq!(
            lines[6], "  ↑↓ move   enter ssh   esc quit   F1 keys",
            "two matches, three rows, none hidden"
        );
    }

    #[test]
    fn the_hint_row_is_dim_text_at_column_two_and_offers_the_key_that_works_now() {
        let (screen, mut picker) = setup("ui", "");
        let mut small = terminal(60, 7);
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[6], format!("{KEYS:<57}1–3"));
        assert!(!dim(&cell(&small, 0, 6)) && !dim(&cell(&small, 1, 6)));
        assert!((2..41).all(|x| dim(&cell(&small, x, 6))));
        assert!((41..57).all(|x| !dim(&cell(&small, x, 6))));
        assert!((57..60).all(|x| dim(&cell(&small, x, 6))));

        picker.handle(Key::Char('c'));
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        assert!(
            lines[6].starts_with("  ↑↓ move   enter ssh   esc quit   F1 keys"),
            "? is a character once there is a query: {:?}",
            lines[6]
        );

        picker.handle(Key::F(1));
        let lines = render(&mut terminal(80, 24), &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines.last().map(String::as_str),
            Some("  ↑↓ move   enter ssh   esc close"),
            "Esc closes the help first, and the help key is not offered"
        );
    }

    #[test]
    fn the_route_row_says_where_the_selected_host_goes() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(80, 9);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(lines[7], "  → ubuntu@10.11.212.191:22 via newstg-bastion");
        let dimmed: Vec<u16> = (0..80).filter(|x| dim(&cell(&terminal, *x, 7))).collect();
        assert_eq!(
            dimmed,
            [2, 28, 29, 30],
            "the arrow and `via`; the route is plain"
        );
        assert!(lines[8].starts_with("  ↑↓ move"), "the hint row stays last");
    }

    #[test]
    fn the_route_row_keeps_the_pieces_ssh_printed_and_dims_every_joining_word() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 9);
        let preview = Preview::parse(
            "remotecommand sudo -i\nproxyjump a,b\nport 2222\nhostname h\nuser ops\n",
        );
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(preview));
        assert_eq!(lines[7], "  → ops@h:2222 via a,b runs sudo -i");
        let dimmed: Vec<u16> = (0..80).filter(|x| dim(&cell(&terminal, *x, 7))).collect();
        assert_eq!(
            dimmed,
            [2, 15, 16, 17, 23, 24, 25, 26],
            "→, via and runs only"
        );

        let bare = Preview::parse("hostname h\n");
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(bare));
        assert_eq!(lines[7], "  → h");
        let none = Preview::parse("user ubuntu\nport 22\n");
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(none));
        assert_eq!(lines[7], "", "no hostname, no route");
    }

    #[test]
    fn the_route_row_says_resolving_while_ssh_has_not_answered() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 9);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[7], "  → resolving…");
        assert!((2..14).all(|x| dim(&cell(&terminal, x, 7))));
        assert!(!dim(&cell(&terminal, 14, 7)));
    }

    #[test]
    fn the_route_row_names_why_ssh_failed_in_the_first_line() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 9);
        let bad = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "/home/me/.ssh/config line 12: Bad configuration option: foo\n\
                     /home/me/.ssh/config: terminating, 1 bad configuration options"
                .to_owned(),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &bad);
        assert_eq!(
            lines[7],
            "  → ssh -G failed: /home/me/.ssh/config line 12: Bad configuration option: foo"
        );
        let lines = render(
            &mut terminal,
            &screen,
            &mut picker,
            &Slot::Failed(ResolveError::Timeout),
        );
        assert_eq!(lines[7], "  → ssh -G gave no answer within 2 s");
    }

    #[test]
    fn a_route_wider_than_the_row_is_cut_inside_the_styled_word_it_lands_in() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(30, 9);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(lines[7], "  → ubuntu@10.11.212.191:22 v…");
        assert!(
            dim(&cell(&terminal, 28, 7)),
            "the `v` of `via` is still dim"
        );
        assert!(!dim(&cell(&terminal, 29, 7)), "the cut is plain");

        let lines = render(&mut self::terminal(3, 9), &screen, &mut picker, &ready());
        assert_eq!(lines[7], "  …");
    }

    #[test]
    fn control_characters_in_what_ssh_printed_never_reach_the_screen() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 9);
        let hostile = Preview::parse("user a\x07b\nhostname c\x1b[31md\nproxyjump e\tf\n");
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(hostile));
        assert!(!lines[7].is_empty());
        assert!(!lines.concat().chars().any(char::is_control), "{lines:#?}");
        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "bad \x1b[31moption\x07".to_owned(),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert_eq!(lines[7], "  → ssh -G failed: bad  [31moption");
    }

    /// The panel's column: everything right of the rule and the space after it.
    fn panel(lines: &[String], cols: u16) -> Vec<String> {
        let skip = usize::from(cols - cols / 5 * 2 + 2);
        lines
            .iter()
            .map(|line| line.chars().skip(skip).collect())
            .collect()
    }

    #[test]
    fn from_100_columns_the_route_is_a_panel_beside_the_list() {
        let (screen, mut picker) = setup("ui", "");
        let mut wide = terminal(120, 30);
        let lines = render(&mut wide, &screen, &mut picker, &ready());
        assert_snapshot!("panel_120x30", frame(&lines));
        assert_snapshot!("panel_120x30_styles", styles(&wide));
        wide.backend_mut().assert_cursor_position((2, 0));

        let (screen, mut picker) = setup("ui", "consul");
        let lines = render(&mut terminal(100, 24), &screen, &mut picker, &ready());
        assert_snapshot!("panel_100x24_consul", frame(&lines));
        assert_eq!(
            panel(&lines, 100)[1..6],
            [
                "newstg-consul1",
                "user       ubuntu",
                "hostname   10.11.212.191",
                "port       22",
                "proxyjump  newstg-bastion",
            ],
            "the rows `sshpick inspect` prints under Resolved"
        );
    }

    #[test]
    fn the_panel_follows_the_selection_and_says_resolving_until_ssh_answers() {
        let (screen, mut picker) = setup("ui", "");
        let mut wide = terminal(100, 8);
        let lines = render(&mut wide, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(panel(&lines, 100)[1..4], ["lab-a", "resolving…", ""]);
        assert!(
            (62..72).all(|x| dim(&cell(&wide, x, 2))),
            "resolving… is dim"
        );
        assert!(!dim(&cell(&wide, 62, 1)), "the host is plain");
        assert!(dim(&cell(&wide, 60, 1)), "the rule is dim");

        picker.handle(Key::Down);
        let lines = render(&mut wide, &screen, &mut picker, &ready());
        assert_eq!(panel(&lines, 100)[1..3], ["lab-b", "user       ubuntu"]);
        assert!((62..66).all(|x| dim(&cell(&wide, x, 2))), "the key is dim");
        assert!(
            (73..79).all(|x| !dim(&cell(&wide, x, 2))),
            "the value is plain"
        );
    }

    #[test]
    fn keys_are_padded_to_the_longest_shown_and_a_long_value_ends_in_an_ellipsis() {
        let (screen, mut picker) = setup("ui", "");
        let preview = Preview::parse(&format!(
            "user ops\nhostname {}.compute.internal\nport 2222\nremotecommand sudo -i\n",
            "ip-10-0-1-23.ap-southeast-1"
        ));
        let lines = render(
            &mut terminal(100, 8),
            &screen,
            &mut picker,
            &Slot::Ready(preview),
        );
        assert_eq!(
            panel(&lines, 100)[2..6],
            [
                "user           ops",
                "hostname       ip-10-0-1-23.ap-southe…",
                "port           2222",
                "remotecommand  sudo -i",
            ]
        );
        let bare = Preview::parse("user ubuntu\nport 22\n");
        let lines = render(
            &mut terminal(100, 8),
            &screen,
            &mut picker,
            &Slot::Ready(bare),
        );
        assert_eq!(
            panel(&lines, 100)[1..3],
            ["lab-a", ""],
            "no hostname, no route"
        );
    }

    #[test]
    fn a_failure_wraps_between_words_and_what_does_not_fit_ends_in_an_ellipsis() {
        let (screen, mut picker) = setup("ui", "");
        let mut wide = terminal(100, 8);
        let bad = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "/home/me/.ssh/config line 12: Bad configuration option: foo\n\
                     /home/me/.ssh/config: terminating, 1 bad configuration options"
                .to_owned(),
        });
        let lines = render(&mut wide, &screen, &mut picker, &bad);
        assert_eq!(
            panel(&lines, 100)[1..5],
            [
                "lab-a",
                "ssh -G failed: /home/me/.ssh/config",
                "line 12: Bad configuration option: foo",
                "",
            ]
        );
        let long = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: format!("{}{}", "word ".repeat(60), "x".repeat(90)),
        });
        let lines = render(&mut wide, &screen, &mut picker, &long);
        let rows = panel(&lines, 100);
        assert_eq!(rows[2], "ssh -G failed: word word word word", "{rows:#?}");
        assert_eq!(
            rows[6], "word word word word word word word wo…",
            "{rows:#?}"
        );
        assert_eq!(rows[7], "", "the hint row is not the panel's");
    }

    #[test]
    fn control_characters_in_what_ssh_printed_never_reach_the_panel() {
        let (screen, mut picker) = setup("ui", "");
        let mut wide = terminal(120, 8);
        let hostile = Preview::parse("user a\x07b\nhostname c\x1b[31md\nproxyjump e\tf\n");
        let lines = render(&mut wide, &screen, &mut picker, &Slot::Ready(hostile));
        assert_eq!(panel(&lines, 120)[2], "user       a b");
        assert!(!lines.concat().chars().any(char::is_control), "{lines:#?}");
    }

    #[test]
    fn a_short_terminal_is_filled_with_the_hint_on_the_last_row() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("short_80x12", frame(&lines));
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    fn rows_of(lines: &[String]) -> String {
        lines
            .iter()
            .map(|line| match line {
                line if line.starts_with('>') => '>',
                line if line.starts_with("  ENV") => 'H',
                line if line.starts_with("  →") => 'R',
                line if line.starts_with("  ↑↓") || line.starts_with("  enter") => 'K',
                _ => 'L',
            })
            .collect()
    }

    #[test_case(80, 9, ">HLLLLLRK"; "everything the hosts need")]
    #[test_case(40, 8, ">HLLLLRK"; "a host fewer")]
    #[test_case(80, 7, ">HLLLRK"; "the shortest with every row")]
    #[test_case(80, 6, ">LLLRK"; "the header goes first")]
    #[test_case(100, 5, ">LLLK"; "then the route")]
    #[test_case(80, 4, ">LLK"; "then a third host")]
    #[test_case(80, 3, ">LK"; "enter and esc outrank a second host")]
    #[test_case(80, 2, ">L"; "then the hint")]
    #[test_case(80, 1, ">"; "the prompt alone")]
    fn a_shorter_terminal_gets_fewer_rows_of_the_same_block(cols: u16, rows: u16, want: &str) {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(cols, rows);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(rows_of(&lines), want, "{lines:#?}");
        if let Some(first) = want.find('L') {
            let y = u16::try_from(first).unwrap();
            assert!(
                (0..cols).all(|x| reversed(&cell(&terminal, x, y))),
                "the first host row is the selection"
            );
        }
    }

    #[test]
    fn a_one_row_block_names_the_selected_host_where_the_count_was() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(30, 1);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(lines, ["> type to filter         lab-a"]);
        assert_eq!(
            styles(&terminal),
            "y0  BOLD[0..0] DIM[2..15]",
            "the host is plain"
        );
        terminal.backend_mut().assert_cursor_position((2, 0));

        picker.handle(Key::Down);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(
            lines,
            ["> type to filter         lab-b"],
            "it follows the selection"
        );

        for c in "zzzz".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(
            lines,
            [format!("{:<27}0/5", "> zzzz")],
            "no selection, the count"
        );
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
            if width >= 1 {
                assert!(lines[0].starts_with('>'), "{width}x{height}: {lines:#?}");
            }
        }
        let lines = render(&mut terminal(30, 1), &screen, &mut picker, &Slot::Waiting);
        assert!(lines[0].starts_with("> consul"), "the prompt still draws");
    }
}
