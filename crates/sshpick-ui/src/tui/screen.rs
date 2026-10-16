use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, Borders};
use sshpick_core::{Entry, Picker, Warning};

use super::pane::{self, Pane};
use super::popup::popup;
use super::size::{
    PANE_LEFT_PADDING, PANE_WIDTH, STACKED_BODY_ROWS, STACKED_TITLE_ROWS, Tier, tier,
};
use super::table::{FOOTER_ROWS, HostTable, footer, prompt_row};

#[derive(Debug)]
pub struct Screen {
    table: HostTable,
    warnings: Vec<String>,
}

impl Screen {
    pub fn new(entries: &[Entry], warnings: &[Warning]) -> Screen {
        Screen {
            table: HostTable::new(entries),
            warnings: warnings.iter().map(ToString::to_string).collect(),
        }
    }

    pub fn draw(&self, frame: &mut Frame, picker: &mut Picker, pane: Pane<'_>) {
        let area = frame.area();
        let tier = tier(area);
        let framed = tier.framed();
        let (left, right) = if tier == Tier::Side {
            let [left, right] = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Length(PANE_WIDTH + PANE_LEFT_PADDING),
            ])
            .areas(area);
            (left, Some(right))
        } else {
            (area, None)
        };
        let content = if right.is_some() {
            let block = Block::new().borders(Borders::RIGHT);
            let inner = block.inner(left);
            frame.render_widget(block, left);
            inner
        } else {
            left
        };
        let [prompt, header, list, stacked, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(u16::from(framed)),
            Constraint::Fill(1),
            Constraint::Length(if tier == Tier::Stacked {
                STACKED_BODY_ROWS + STACKED_TITLE_ROWS
            } else {
                0
            }),
            Constraint::Length(if framed { FOOTER_ROWS } else { 0 }),
        ])
        .areas(content);

        picker.follow(list.height.into());
        let view = picker.view(list.height.into());
        prompt_row(frame, prompt, &view);
        self.table.list(frame, list, &view);
        if framed {
            self.table.header(frame, header);
            footer(frame, footer_area, &view, self.warnings.len());
        }
        if let Some(right) = right {
            let [_, preview] =
                Layout::horizontal([Constraint::Length(PANE_LEFT_PADDING), Constraint::Fill(1)])
                    .areas(right);
            pane::side(frame, preview, pane, view.matched);
        }
        if tier == Tier::Stacked {
            pane::stacked(frame, stacked, pane);
        }
        if picker.help_open() {
            popup(frame, &self.warnings);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use insta::assert_snapshot;
    use ratatui::style::Color;
    use sshpick_core::{Key, Preview, ResolveError, Slot, Warning};

    use super::*;
    use crate::tui::testing::{
        bold, cell, dim, frame, ready, render, reversed, setup, setup_with, styles, terminal,
    };

    #[test]
    fn the_reference_screens_are_drawn_cell_for_cell() {
        let (screen, mut picker) = setup("ui", "");
        let mut wide = terminal(120, 30);
        let lines = render(&mut wide, &screen, &mut picker, &ready());
        assert_snapshot!("side_120x30", frame(&lines));
        assert_snapshot!("side_120x30_styles", styles(&wide));
        wide.backend_mut().assert_cursor_position((2, 0));

        let (screen, mut picker) = setup("ui", "consul");
        let mut stacked = terminal(80, 24);
        let lines = render(&mut stacked, &screen, &mut picker, &ready());
        assert_snapshot!("stacked_80x24_consul", frame(&lines));
        assert_snapshot!("stacked_80x24_consul_styles", styles(&stacked));
        stacked.backend_mut().assert_cursor_position((2 + 6, 0));
    }

    #[test]
    fn rows_list_top_down_under_the_header_and_the_count_sits_on_the_prompt() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("stacked_60x20", frame(&lines));
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
        let warnings = [Warning::Cycle {
            path: PathBuf::from("/x"),
        }];
        for (query, width, height) in [("", 120, 30), ("consul", 80, 24), ("zzzz", 100, 12)] {
            let (screen, mut picker) = setup_with("ui", query, &warnings);
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
    fn the_side_tier_has_one_border_the_right_edge_of_the_list() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(100, 16);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("side_100x16_consul", frame(&lines));
        for y in 0..16 {
            assert_eq!(cell(&terminal, 58, y).symbol(), "│", "row {y}");
            assert!(!dim(&cell(&terminal, 58, y)), "the border is plain");
        }
        let borders = lines
            .iter()
            .flat_map(|line| line.chars())
            .filter(|c| "│─┌┐└┘".contains(*c));
        assert_eq!(borders.count(), 16, "no other rule or box");
        assert!((0..58).all(|x| reversed(&cell(&terminal, x, 2))));
        assert!(!reversed(&cell(&terminal, 59, 2)) && !reversed(&cell(&terminal, 60, 2)));
        assert!(dim(&cell(&terminal, 61, 0)), "the pane title is dim");
        assert_eq!(
            cell(&terminal, 60, 1).symbol(),
            "u",
            "the body starts at the pane's edge"
        );
        assert!(dim(&cell(&terminal, 60, 1)), "keys are dim");
        assert!(!dim(&cell(&terminal, 74, 1)), "values are plain");
        terminal.backend_mut().assert_cursor_position((2 + 6, 0));
    }

    #[test]
    fn the_other_tiers_have_no_border_at_all() {
        for (width, height) in [(80, 24), (80, 12), (99, 20), (60, 6)] {
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
            let wide = width >= 100;
            let list_width = if wide { width - 42 } else { width };
            let mut terminal = terminal(width, 20);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            assert!(lines[0].starts_with("> type to filter"), "{width}");
            assert!(lines[0].contains("5/5"), "{width}");
            assert!(lines[2].starts_with("▌ LAB         lab-a"), "{width}");
            let select = if wide {
                format!("{:<w$}│", "↑ ↓ - Select", w = usize::from(list_width))
            } else {
                "↑ ↓ - Select".to_owned()
            };
            assert_eq!(lines[18], select, "{width}");
            assert!(lines[19].starts_with("? - More binds"), "{width}");
            assert_eq!(lines[19].ends_with("Esc - Quit"), wide, "{width}");
            assert!((0..list_width).all(|x| reversed(&cell(&terminal, x, 2))));
            assert!((list_width..width).all(|x| !reversed(&cell(&terminal, x, 2))));
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
    fn zero_matches_say_no_match_on_the_first_row_and_the_footer_offers_only_quit() {
        let warnings = [Warning::Cycle {
            path: PathBuf::from("/x"),
        }];
        let (screen, mut picker) = setup_with("ui", "zzzz", &warnings);
        let mut wide = terminal(120, 14);
        let lines = render(&mut wide, &screen, &mut picker, &ready());
        assert_snapshot!("no_match_120x14", frame(&lines));
        assert!((2..10).all(|x| dim(&cell(&wide, x, 2))), "no match is dim");
        assert!((0..120).all(|x| !reversed(&cell(&wide, x, 2))));
        assert!(!lines.iter().any(|line| line.contains("Enter - ssh")));
    }

    #[test]
    fn hidden_rows_are_counted_in_the_footer() {
        let (screen, mut picker) = setup("ui", "");
        let mut below = |height: u16| {
            let mut terminal = terminal(60, height);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            lines[usize::from(height) - 2].clone()
        };
        for height in [16, 15, 12, 9] {
            assert_eq!(below(height), "↑ ↓ - Select", "{height}: all five fit");
        }
        assert_eq!(below(14), "↑ ↓ - Select   1 more below");
        assert_eq!(below(13), "↑ ↓ - Select   2 more below");
        assert_eq!(below(8), "↑ ↓ - Select   1 more below");
        assert_eq!(below(7), "↑ ↓ - Select   2 more below");

        let mut scrolled = terminal(60, 13);
        for _ in 0..4 {
            picker.handle(Key::Down);
        }
        let lines = render(&mut scrolled, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[2],
            "  NEW STAGING newstg-consul1 consul server 1 — mgmt"
        );
        assert_eq!(lines[4], "▌ NEW STAGING newstg-bastion bastion");
        assert_eq!(
            lines[11], "↑ ↓ - Select",
            "scrolled to the end, nothing below"
        );

        let (screen, mut picker) = setup("ui", "'lab");
        let lines = render(&mut terminal(60, 13), &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[2], "▌ LAB         lab-b          build box");
        assert_eq!(lines[3], "  LAB         lab-a          LAB — personal VMs");
        assert_eq!(
            lines[11], "↑ ↓ - Select",
            "two matches, three rows, none hidden"
        );
    }

    #[test]
    fn the_footer_offers_the_key_that_works_now_and_counts_warnings() {
        let (screen, mut picker) = setup("ui", "");
        let last = |picker: &mut Picker, screen: &Screen| {
            let lines = render(&mut terminal(60, 14), screen, picker, &Slot::Waiting);
            lines[13].clone()
        };
        assert_eq!(last(&mut picker, &screen), "? - More binds");
        picker.handle(Key::Char('c'));
        assert_eq!(last(&mut picker, &screen), "F1 - More binds");

        let warnings = [Warning::Cycle {
            path: PathBuf::from("/x"),
        }];
        let (screen, mut picker) = setup_with("home", "", &warnings);
        let lines = render(&mut terminal(100, 12), &screen, &mut picker, &Slot::Waiting);
        assert!(
            lines[11].starts_with("? - More binds   1 warning "),
            "{:?}",
            lines[11]
        );
        assert!(
            !lines.concat().contains("include cycle"),
            "the text is in the popup"
        );
    }

    #[test]
    fn the_wide_pane_shows_one_more_below_on_a_short_list() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 8);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("side_100x8_one_below", frame(&lines));
        assert!(dim(&cell(&terminal, 5, 6)));
        assert!(dim(&cell(&terminal, 70, 7)), "the pane footer is dim");
    }

    #[test]
    fn the_wide_pane_cuts_long_values_and_wraps_errors_inside_its_40_columns() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 16);
        let long = Slot::Ready(Preview::parse(
            "user ubuntu\nproxyjump a-very-long-bastion-host-name\n",
        ));
        let lines = render(&mut terminal, &screen, &mut picker, &long);
        assert!(
            lines[2].ends_with("proxyjump     a-very-long-bastion-host-…"),
            "{:?}",
            lines[2]
        );
        assert_eq!(lines[2].chars().count(), 100);
        assert_eq!(cell(&terminal, 99, 2).symbol(), "…");

        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "word ".repeat(30),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert!(
            lines[0].ends_with("lab-a > ssh -G failed"),
            "{:?}",
            lines[0]
        );
        for line in &lines[1..5] {
            let pane: String = line.chars().skip(60).collect();
            assert!(
                pane.starts_with("word") && pane.chars().count() <= 40,
                "{pane:?}"
            );
        }
        assert!(lines[5].ends_with('│'), "{:?}", lines[5]);
    }

    #[test]
    fn a_waiting_pane_shows_its_title_and_no_selection_shows_only_the_footer() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(100, 16);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!(
            lines[0].ends_with("│  newstg-consul1 > ssh -G »"),
            "{:?}",
            lines[0]
        );
        assert!(
            lines[1..14].iter().all(|line| line.ends_with('│')),
            "{lines:#?}"
        );
        assert!(lines[15].ends_with("Enter - ssh   Esc - Quit"));

        for c in "zzzz".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!(lines[1..14].iter().all(|line| line.ends_with('│')));
        assert!(!lines.concat().contains("ssh -G"), "{lines:#?}");
        assert!(lines[15].ends_with("│    Esc - Quit"), "{:?}", lines[15]);
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
    fn a_100x6_terminal_keeps_only_the_prompt_and_the_list() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 6);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("bare_100x6", frame(&lines));
        assert!((0..100).all(|x| reversed(&cell(&terminal, x, 1))));
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
        let title = |line: &str| line.starts_with(" lab-a > ssh -G");
        let header = |line: &str| line.starts_with("  ENV         HOST           WHAT");
        let footer = |line: &str| line.starts_with("? - More binds");

        assert!(has(&lines_at(80, 13), &title));
        assert!(!has(&lines_at(80, 12), &title));
        assert!(
            !has(&lines_at(100, 40), &title),
            "wide has its title on the prompt row, not a row of its own"
        );
        for width in [80, 100] {
            for (height, shown) in [(7, true), (6, false)] {
                let lines = lines_at(width, height);
                assert_eq!(has(&lines, &header), shown, "{width}x{height}");
                assert_eq!(has(&lines, &footer), shown, "{width}x{height}");
            }
        }
        let lines = lines_at(80, 7);
        assert!(lines[0].starts_with("> "));
        assert!(lines[2].starts_with("▌ LAB"));
        assert!(lines[3].starts_with("  LAB         lab-b"));
        assert_eq!(lines[5], "↑ ↓ - Select   2 more below");
        assert_eq!(lines[6], "? - More binds");
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
