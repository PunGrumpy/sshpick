use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use sshpick_core::{Entry, Picker, Slot};

use super::help::{self, help};
use super::route::route;
use super::table::{HostTable, hint_row, prompt_row};

const MIN_HEIGHT_WITH_FRAME: u16 = 7;
const LIST_KEPT_WITH_HELP: u16 = 3;

#[derive(Debug)]
pub struct Screen {
    table: HostTable,
}

impl Screen {
    pub fn new(entries: &[Entry]) -> Screen {
        Screen {
            table: HostTable::new(entries),
        }
    }

    /// `slot` is the selected host's preview, `None` when nothing matches.
    pub fn draw(&self, frame: &mut Frame, picker: &mut Picker, slot: Option<&Slot>) {
        let area = frame.area();
        let framed = u16::from(area.height >= MIN_HEIGHT_WITH_FRAME);
        let room = area.height.saturating_sub(1 + 3 * framed);
        let help_rows = if picker.help_open() {
            help::rows().min(room.saturating_sub(LIST_KEPT_WITH_HELP))
        } else {
            0
        };
        let [prompt, header, list, route_area, help_area, hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(framed),
            Constraint::Fill(1),
            Constraint::Length(framed),
            Constraint::Length(help_rows),
            Constraint::Length(framed),
        ])
        .areas(area);

        picker.follow(list.height.into());
        let view = picker.view(list.height.into());
        prompt_row(frame, prompt, &view);
        self.table.list(frame, list, &view);
        self.table.header(frame, header);
        route(frame, route_area, slot);
        help(frame, help_area);
        hint_row(frame, hint_area, &view);
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use ratatui::style::Color;
    use sshpick_core::{Key, Preview, ResolveError, Slot};

    use crate::tui::testing::{
        bold, cell, dim, frame, ready, render, reversed, setup, styles, terminal,
    };

    #[test]
    fn the_reference_screens_are_drawn_cell_for_cell() {
        let (screen, mut picker) = setup("ui", "");
        let mut wide = terminal(120, 30);
        let lines = render(&mut wide, &screen, &mut picker, &ready());
        assert_snapshot!("empty_120x30", frame(&lines));
        assert_snapshot!("empty_120x30_styles", styles(&wide));
        wide.backend_mut().assert_cursor_position((2, 0));

        let (screen, mut picker) = setup("ui", "consul");
        let mut default = terminal(80, 24);
        let lines = render(&mut default, &screen, &mut picker, &ready());
        assert_snapshot!("consul_80x24", frame(&lines));
        assert_snapshot!("consul_80x24_styles", styles(&default));
        default.backend_mut().assert_cursor_position((2 + 6, 0));
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
        let mut terminal = terminal(40, 20);
        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: format!("{}\nsecond line", "word ".repeat(60)),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert_eq!(lines[18], "  → ssh -G failed: word word word word …");
        assert!(!lines.concat().contains("second line"));
        assert!(lines[17].is_empty(), "nothing wraps onto the rows above");
        assert!(dim(&cell(&terminal, 2, 18)) && !dim(&cell(&terminal, 4, 18)));
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
    fn there_is_no_border_or_rule_at_any_size() {
        for (width, height) in [(80, 24), (80, 12), (99, 20), (100, 24), (120, 30), (60, 6)] {
            let (screen, mut picker) = setup("ui", "");
            let lines = render(&mut terminal(width, height), &screen, &mut picker, &ready());
            let drawn: String = lines.concat();
            assert!(
                !drawn.contains(['│', '─', '┌', '┐', '└', '┘']),
                "{width}x{height}: {lines:#?}"
            );
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
        for width in [60, 99, 100, 101, 160, 200] {
            let mut terminal = terminal(width, 20);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            assert!(lines[0].starts_with("> type to filter"), "{width}");
            assert!(lines[0].ends_with("5/5"), "{width}");
            assert_eq!(lines[0].chars().count(), usize::from(width));
            assert!(lines[2].starts_with("▌ LAB         lab-a"), "{width}");
            assert_eq!(lines[18], "  → resolving…", "{width}");
            assert_eq!(
                lines[19], "  ↑↓ move   enter ssh   esc quit   ? keys",
                "{width}"
            );
            assert!((0..width).all(|x| reversed(&cell(&terminal, x, 2))));
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
        let mut wide = terminal(120, 14);
        let lines = render(&mut wide, &screen, &mut picker, &ready());
        assert_snapshot!("no_match_120x14", frame(&lines));
        assert_eq!(lines[2], "  no matching hosts");
        assert!((0..19).all(|x| dim(&cell(&wide, x, 2))), "no match is dim");
        assert!((19..120).all(|x| !dim(&cell(&wide, x, 2))));
        assert!((0..120).all(|x| !reversed(&cell(&wide, x, 2))));
        assert!(lines[12].is_empty(), "there is no route to show");
        assert_eq!(lines[13], "  esc quit   F1 keys", "no enter, no move");
    }

    #[test]
    fn the_window_range_sits_flush_right_on_the_hint_row_when_the_list_scrolls() {
        let (screen, mut picker) = setup("ui", "");
        let keys = "  ↑↓ move   enter ssh   esc quit   ? keys";
        let mut last = |height: u16| {
            let mut terminal = terminal(60, height);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            lines[usize::from(height) - 1].clone()
        };
        for height in [24, 12, 10, 9] {
            assert_eq!(last(height), keys, "{height}: all five fit");
        }
        assert_eq!(last(8), format!("{keys:<57}1–4"));
        assert_eq!(last(7), format!("{keys:<57}1–3"));

        let mut scrolled = terminal(60, 8);
        for _ in 0..4 {
            picker.handle(Key::Down);
        }
        let lines = render(&mut scrolled, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[2], "  LAB         lab-b          build box");
        assert_eq!(lines[5], "▌ NEW STAGING newstg-bastion bastion");
        assert_eq!(lines[7], format!("{keys:<57}2–5"), "scrolled to the end");

        let (screen, mut picker) = setup("ui", "'lab");
        let lines = render(&mut terminal(60, 8), &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[2], "▌ LAB         lab-b          build box");
        assert_eq!(lines[3], "  LAB         lab-a          LAB — personal VMs");
        assert_eq!(
            lines[7], "  ↑↓ move   enter ssh   esc quit   F1 keys",
            "two matches, four rows, none hidden"
        );
    }

    #[test]
    fn the_hint_row_is_dim_text_at_column_two_and_offers_the_key_that_works_now() {
        let (screen, mut picker) = setup("ui", "");
        let mut small = terminal(60, 8);
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[7],
            format!("{:<57}1–4", "  ↑↓ move   enter ssh   esc quit   ? keys")
        );
        assert!(!dim(&cell(&small, 0, 7)) && !dim(&cell(&small, 1, 7)));
        assert!((2..41).all(|x| dim(&cell(&small, x, 7))));
        assert!((41..57).all(|x| !dim(&cell(&small, x, 7))));
        assert!((57..60).all(|x| dim(&cell(&small, x, 7))));

        picker.handle(Key::Char('c'));
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        assert!(
            lines[7].starts_with("  ↑↓ move   enter ssh   esc quit   F1 keys"),
            "? is a character once there is a query: {:?}",
            lines[7]
        );

        picker.handle(Key::F(1));
        let lines = render(&mut terminal(80, 24), &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[23], "  ↑↓ move   enter ssh   esc close",
            "Esc closes the help first, and the help key is not offered"
        );
    }

    #[test]
    fn the_route_row_says_where_the_selected_host_goes() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(lines[22], "  → ubuntu@10.11.212.191:22 via newstg-bastion");
        let dimmed: Vec<u16> = (0..80).filter(|x| dim(&cell(&terminal, *x, 22))).collect();
        assert_eq!(
            dimmed,
            [2, 28, 29, 30],
            "the arrow and `via`; the route is plain"
        );
        assert!(
            lines[23].starts_with("  ↑↓ move"),
            "the hint row stays last"
        );
    }

    #[test]
    fn the_route_row_keeps_the_pieces_ssh_printed_and_dims_every_joining_word() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let preview = Preview::parse(
            "remotecommand sudo -i\nproxyjump a,b\nport 2222\nhostname h\nuser ops\n",
        );
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(preview));
        assert_eq!(lines[22], "  → ops@h:2222 via a,b runs sudo -i");
        let dimmed: Vec<u16> = (0..80).filter(|x| dim(&cell(&terminal, *x, 22))).collect();
        assert_eq!(
            dimmed,
            [2, 15, 16, 17, 23, 24, 25, 26],
            "→, via and runs only"
        );

        let bare = Preview::parse("hostname h\n");
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(bare));
        assert_eq!(lines[22], "  → h");
        let none = Preview::parse("user ubuntu\nport 22\n");
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(none));
        assert_eq!(lines[22], "", "no hostname, no route");
    }

    #[test]
    fn the_route_row_says_resolving_while_ssh_has_not_answered() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[22], "  → resolving…");
        assert!((2..14).all(|x| dim(&cell(&terminal, x, 22))));
        assert!(!dim(&cell(&terminal, 14, 22)));
    }

    #[test]
    fn the_route_row_names_why_ssh_failed_in_the_first_line() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let bad = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "/home/me/.ssh/config line 12: Bad configuration option: foo\n\
                     /home/me/.ssh/config: terminating, 1 bad configuration options"
                .to_owned(),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &bad);
        assert_eq!(
            lines[22],
            "  → ssh -G failed: /home/me/.ssh/config line 12: Bad configuration option: foo"
        );
        let lines = render(
            &mut terminal,
            &screen,
            &mut picker,
            &Slot::Failed(ResolveError::Timeout),
        );
        assert_eq!(lines[22], "  → ssh -G gave no answer within 2 s");
    }

    #[test]
    fn a_route_wider_than_the_row_is_cut_inside_the_styled_word_it_lands_in() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(30, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(lines[22], "  → ubuntu@10.11.212.191:22 v…");
        assert!(
            dim(&cell(&terminal, 28, 22)),
            "the `v` of `via` is still dim"
        );
        assert!(!dim(&cell(&terminal, 29, 22)), "the cut is plain");

        let lines = render(&mut self::terminal(3, 24), &screen, &mut picker, &ready());
        assert_eq!(lines[22], "  …");
    }

    #[test]
    fn control_characters_in_what_ssh_printed_never_reach_the_screen() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let hostile = Preview::parse("user a\x07b\nhostname c\x1b[31md\nproxyjump e\tf\n");
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(hostile));
        assert!(!lines[22].is_empty());
        assert!(!lines.concat().chars().any(char::is_control), "{lines:#?}");
        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "bad \x1b[31moption\x07".to_owned(),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert_eq!(lines[22], "  → ssh -G failed: bad  [31moption");
    }

    #[test]
    fn an_80x12_terminal_gives_the_list_the_rows_between_the_header_and_the_route() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("short_80x12", frame(&lines));
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn a_100x6_terminal_keeps_only_the_prompt_and_the_list() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 6);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("bare_100x6", frame(&lines));
        assert!((0..100).all(|x| reversed(&cell(&terminal, x, 1))));
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn the_frame_needs_7_rows_and_below_that_only_the_prompt_and_the_list_remain() {
        let (screen, mut picker) = setup("ui", "");
        let mut lines_at = |width, height| {
            let mut terminal = terminal(width, height);
            render(&mut terminal, &screen, &mut picker, &ready())
        };
        let has =
            |lines: &[String], wanted: &dyn Fn(&str) -> bool| lines.iter().any(|line| wanted(line));
        let header = |line: &str| line.starts_with("  ENV         HOST           WHAT");
        let route = |line: &str| line.starts_with("  → ubuntu@");
        let hint = |line: &str| line.starts_with("  ↑↓ move");

        for width in [40, 80, 100] {
            for (height, shown) in [(7, true), (6, false)] {
                let lines = lines_at(width, height);
                assert_eq!(has(&lines, &header), shown, "{width}x{height}");
                assert_eq!(has(&lines, &route), shown, "{width}x{height}");
                assert_eq!(has(&lines, &hint), shown, "{width}x{height}");
            }
        }
        let lines = lines_at(80, 7);
        assert!(lines[0].starts_with("> "));
        assert!(lines[2].starts_with("▌ LAB"));
        assert!(lines[3].starts_with("  LAB         lab-b"));
        assert_eq!(
            lines[4],
            "  NEW STAGING newstg-consul1 consul server 1 — mgmt"
        );
        assert_eq!(lines[5], "  → ubuntu@10.11.212.191:22 via newstg-bastion");
        assert_eq!(
            lines[6],
            format!("{:<77}1–3", "  ↑↓ move   enter ssh   esc quit   ? keys")
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
            if height >= 2 && width >= 1 {
                let prompt = &lines[0];
                assert!(prompt.starts_with('>'), "{width}x{height}: {lines:#?}");
            }
        }
        let lines = render(&mut terminal(30, 1), &screen, &mut picker, &Slot::Waiting);
        assert!(lines[0].starts_with("> consul"), "the prompt still draws");
    }
}
