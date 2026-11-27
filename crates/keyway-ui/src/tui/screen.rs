use keyway_core::{Entry, Picker, Slot};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use super::help::{self, help};
use super::panel::{border, rule};
use super::plan::{Plan, Space, plan};
use super::style::{Role, Theme};
use super::table::{HostTable, hint_row, prompt_row};
use crate::ColorConfig;

#[derive(Debug)]
pub struct Screen {
    table: HostTable,
    theme: Theme,
}

impl Screen {
    /// `colors` is the decision for the stream the picker draws on.
    pub fn new(entries: &[Entry], colors: ColorConfig) -> Screen {
        Screen {
            table: HostTable::new(entries),
            theme: Theme::new(colors),
        }
    }

    pub(super) fn plan(&self, space: Space, picker: &Picker) -> Plan {
        let help = if picker.help_open() { help::rows() } else { 0 };
        plan(space, help)
    }

    /// `frame.area()` is the terminal; the panel `plan` describes sits in it, and the rest is left
    /// blank.
    pub(super) fn draw(
        &self,
        frame: &mut Frame,
        picker: &mut Picker,
        slot: Option<&Slot>,
        plan: Plan,
    ) {
        let panel = plan.panel;
        let outer =
            Rect::new(panel.left, panel.top, panel.cols, panel.rows).intersection(frame.area());
        let inner = if panel.border {
            border(frame, outer, self.theme)
        } else {
            outer
        };
        let [prompt, _gap, list_area, help_area, rule_area, hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(u16::from(plan.gap)),
            Constraint::Length(plan.list),
            Constraint::Length(plan.help),
            Constraint::Length(u16::from(panel.border)),
            Constraint::Length(u16::from(plan.hint)),
        ])
        .areas(inner);
        rule(frame, rule_area, self.theme);

        let room = usize::from(plan.list);
        picker.follow(room);
        let view = picker.view(room);
        // A block with no room for the selected host says it on the prompt row, so Enter is never
        // blind.
        let tail = picker
            .selected()
            .filter(|_| view.rows.is_empty())
            .map(|host| (host.to_string(), Role::Typed));
        prompt_row(frame, prompt, &view, self.theme, tail);
        self.table.list(frame, list_area, &view, self.theme);
        help(frame, help_area, self.theme);
        let slot = slot.filter(|_| plan.hint);
        hint_row(frame, hint_area, &view, self.theme, slot);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use insta::assert_snapshot;
    use keyway_core::{Key, Preview, ResolveError, Slot};
    use ratatui::style::Color;
    use test_case::test_case;

    use crate::tui::testing::{
        Block, bold, cell, dim, is_bar, is_bar_match, is_fail, is_field, is_keycap, is_match,
        is_muted, is_muted_match, is_placeholder, is_plain, is_stroke, is_typed, ready,
        recently_used, recently_what, render, render_panel, reversed, screen_cell, screen_lines,
        setup, setup_colored, setup_with, shown, styles, terminal,
    };

    /// A footer's keys, as the tests spell them: the caps of the widest rung.
    const KEYS: &str = "Keys ?    Quit esc    Connect ↵";
    const PROMPT: &str = "  Search host, ENV or what";

    fn is_bar_row(terminal: &Block, y: u16, color: bool) -> bool {
        let width = terminal.width();
        (1..width - 1).all(|x| is_bar(&cell(terminal, x, y), color))
    }

    fn colored(color: bool, query: &str) -> (super::Screen, keyway_core::Picker) {
        if color {
            setup_colored("ui", query)
        } else {
            setup("ui", query)
        }
    }

    #[test]
    fn the_reference_screens_are_drawn_cell_for_cell() {
        let (screen, mut picker) = setup("ui", "");
        let mut empty = terminal(80, 24);
        render(&mut empty, &screen, &mut picker, &ready());
        assert_snapshot!("empty_80x24", shown(&empty));
        assert_snapshot!("empty_80x24_styles", styles(&empty));
        empty.assert_cursor(2, 0);

        let (screen, mut picker) = setup("ui", "con");
        let mut con = terminal(80, 24);
        render(&mut con, &screen, &mut picker, &ready());
        assert_snapshot!("con_80x24", shown(&con));
        assert_snapshot!("con_80x24_styles", styles(&con));
        con.assert_cursor(5, 0);
    }

    #[test_case(true; "with colour")]
    #[test_case(false; "without")]
    fn the_prompt_is_the_query_then_the_count_on_one_strip(color: bool) {
        let (screen, mut picker) = colored(color, "");
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert!(lines[0].starts_with(PROMPT), "{lines:#?}");
        assert!(lines[0].ends_with("  5 hosts"), "{lines:#?}");
        let at = |x| cell(&terminal, x, 0);
        assert!(
            is_plain(&at(0)) && is_plain(&at(73)),
            "the margins are not the strip"
        );
        assert!(is_field(&at(1), color), "the strip's padding");
        assert!(
            (2..26).all(|x| is_placeholder(&at(x), color)),
            "the placeholder is muted on the field"
        );
        let count = u16::try_from(lines[0].find("5 hosts").unwrap()).unwrap();
        assert!(
            (count..count + 7).all(|x| is_placeholder(&at(x), color)),
            "the count is muted too"
        );
        terminal.assert_cursor(2, 0);

        picker.handle(Key::Char('l'));
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert!(lines[0].starts_with("  l"), "{lines:#?}");
        assert!(lines[0].ends_with("  4 / 5"), "{lines:#?}");
        assert!(
            is_typed(&cell(&terminal, 2, 0), color),
            "the query is the thing"
        );
        terminal.assert_cursor(3, 0);

        picker.handle(Key::Ctrl('u'));
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert!(
            lines[0].starts_with(PROMPT),
            "clearing brings the placeholder back: {lines:#?}"
        );
        terminal.assert_cursor(2, 0);
    }

    #[test]
    fn the_whole_prompt_row_is_a_strip_that_lines_up_with_the_bar() {
        let (screen, mut picker) = setup_colored("ui", "");
        let mut terminal = terminal(80, 24);
        render(&mut terminal, &screen, &mut picker, &ready());
        let on = |y| -> Vec<u16> {
            (0..74)
                .filter(|x| !is_plain(&cell(&terminal, *x, y)))
                .collect()
        };
        let (strip, bar) = (on(0), on(2));
        assert_eq!(strip, (1..73).collect::<Vec<_>>(), "margin to margin");
        assert_eq!(
            (strip.first(), strip.last()),
            (bar.first(), bar.last()),
            "the edges line up with the bar's"
        );
        assert!(
            on(1).is_empty(),
            "the blank row under the prompt is not on it"
        );
        assert!(
            (1..73).all(|x| cell(&terminal, x, 0).bg != ratatui::style::Color::Reset),
            "every column of it has a background"
        );
    }

    #[test]
    fn a_long_query_hides_the_count_and_scrolls_with_the_cursor_in_it() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        for c in "z".repeat(60).chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!(
            !lines[0].contains("0 / 5"),
            "the query took the count's room: {:?}",
            lines[0]
        );
        // Inside the border: a 52-column strip, less the padding, keeps the cursor's cell.
        assert_eq!(
            lines[0],
            format!("  {}", "z".repeat(50)),
            "the query scrolls, with the cursor's cell after it"
        );
        terminal.assert_cursor(52, 0);
    }

    #[test_case(36)]
    fn the_count_goes_before_the_placeholder_is_cut(cols: u16) {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(cols, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!(
            lines[0].starts_with(PROMPT),
            "the placeholder is whole: {:?}",
            lines[0]
        );
        assert!(
            !lines[0].contains("hosts"),
            "the count went, rather than clip the placeholder: {:?}",
            lines[0]
        );
    }

    #[test_case(false)]
    #[test_case(true)]
    fn every_host_is_one_line_and_the_selected_one_is_the_bar(color: bool) {
        let (screen, mut picker) = colored(color, "");
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[1], "", "the blank row under the prompt");
        assert!(lines[2].starts_with("  lab-a") && lines[2].ends_with("LAB"));
        assert!(lines[2].contains("LAB — personal VMs"), "{lines:#?}");
        assert!(lines[3].starts_with("  lab-b") && lines[3].ends_with("LAB"));
        assert!(lines[3].contains("build box"), "{lines:#?}");
        assert!(lines[4].starts_with("  newstg-consul1"));
        assert!(lines[4].contains("consul server 1 — mgmt") && lines[4].ends_with("NEW STAGING"));
        assert!(lines[5].starts_with("  newstg-consul2") && lines[5].ends_with("NEW STAGING"));
        assert!(lines[6].starts_with("  newstg-bastion") && lines[6].ends_with("NEW STAGING"));
        assert!(lines[6].contains("bastion"), "{lines:#?}");

        let at = |x, y| cell(&terminal, x, y);
        assert!(is_bar_row(&terminal, 2, color), "the selected host");
        assert!(
            is_plain(&at(0, 2)) && is_plain(&at(73, 2)),
            "the margins are not the bar"
        );
        for y in 3..17 {
            assert!(
                (0..74).all(|x| !reversed(&at(x, y)) && at(x, y).bg == Color::Reset),
                "nothing below the bar is selected: line {y}"
            );
        }
    }

    #[test_case(false)]
    #[test_case(true)]
    fn off_the_bar_host_is_plain_and_what_and_env_are_muted(color: bool) {
        let (screen, mut picker) = colored(color, "");
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let at = |x, y| cell(&terminal, x, y);
        assert_eq!(at(2, 3).symbol(), "l");
        assert!((2..7).all(|x| is_plain(&at(x, 3))), "HOST is the thing");
        let what = u16::try_from(lines[3].find("build box").unwrap()).unwrap();
        assert!(
            (what..what + 9).all(|x| is_muted(&at(x, 3), color)),
            "WHAT is context"
        );
        assert_eq!(at(69, 3).symbol(), "L");
        assert!(
            (69..72).all(|x| is_muted(&at(x, 3), color)),
            "ENV is context"
        );
        assert!(is_placeholder(&at(2, 0), color), "the placeholder");
    }

    #[test]
    fn recent_hosts_come_first_with_last_before_env_on_the_same_line() {
        let (screen, mut picker) = setup_with("ui", "", &recently_used());
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("recent_80x24", shown(&terminal));
        let at = |x, y| cell(&terminal, x, y);
        assert!(lines[2].starts_with("  newstg-consul1") && lines[2].ends_with("NEW STAGING"));
        assert!(
            lines[2].contains("consul server 1 — mgmt"),
            "WHAT on the row"
        );
        assert!(
            lines[2].contains("2h") && lines[2].contains("  2h  "),
            "LAST before ENV: {:?}",
            lines[2]
        );
        let last = u16::try_from(lines[3].find("3d").unwrap()).unwrap();
        assert!(
            (last..last + 2).all(|x| is_muted(&at(x, 3), false)),
            "LAST is context"
        );
        assert!(lines[3].starts_with("  lab-b") && lines[3].contains("3d"));
        assert!(lines[4].starts_with("  newstg-consul2") && lines[4].contains("2w"));
        assert!(
            lines[5].starts_with("  lab-a")
                && !lines[5].contains(" 5m")
                && !lines[5].contains(" 0"),
            "a host never used has no LAST: {:?}",
            lines[5]
        );
        terminal.assert_cursor(2, 0);
    }

    #[test]
    fn a_query_keeps_last_and_never_bolds_it() {
        let (screen, mut picker) = setup_with("ui", "consul", &recently_used());
        let mut terminal = terminal(100, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("recent_consul_100x12", shown(&terminal));
        let row = lines.iter().position(|row| row.contains("2h")).unwrap() as u16;
        let last = u16::try_from(lines[usize::from(row)].find("2h").unwrap()).unwrap();
        assert!(
            (last..last + 2).all(|x| !bold(&cell(&terminal, x, row))),
            "LAST is never bold"
        );
    }

    #[test]
    fn last_neither_appears_nor_goes_while_the_query_changes() {
        let (screen, mut picker) = setup_with("ui", "", &recently_used());
        let mut terminal = terminal(80, 24);
        let mut rows = Vec::new();
        for query in ["", "consul", "consul1"] {
            picker.handle(Key::Ctrl('u'));
            for c in query.chars() {
                picker.handle(Key::Char(c));
            }
            let lines = render(&mut terminal, &screen, &mut picker, &ready());
            rows.push(lines[2].clone());
        }
        assert!(rows[0].starts_with("  newstg-consul1") && rows[0].ends_with("NEW STAGING"));
        assert!(rows.iter().all(|row| *row == rows[0]), "{rows:#?}");
    }

    //          width, WHAT whole, LAST
    #[test_case(80, true, true; "wide enough for everything")]
    #[test_case(67, true, true; "it all just fits")]
    #[test_case(62, false, true; "WHAT is cut first, LAST stays")]
    #[test_case(47, false, true; "the cut WHAT keeps its least columns")]
    #[test_case(46, false, false; "LAST goes when WHAT would keep fewer than eight")]
    #[test_case(41, false, false; "WHAT alone keeps the least it can")]
    #[test_case(40, false, false; "WHAT goes too")]
    fn what_is_cut_before_last_is_left_out(width: u16, what: bool, last: bool) {
        let (screen, mut picker) = setup_with("ui", "", &recently_used());
        let lines = render(&mut terminal(width, 12), &screen, &mut picker, &ready());
        let row = lines
            .iter()
            .find(|row| row.starts_with("  newstg-consul1"))
            .unwrap();
        assert_eq!(
            row.contains("consul server 1 — mgmt"),
            what,
            "{width}: {row}"
        );
        assert_eq!(row.contains("2h"), last, "{width}: {row}");
        assert!(row.ends_with("NEW STAGING"), "ENV stays: {width}: {row}");
        assert!(row.chars().count() < usize::from(width), "{width}: {row}");
    }

    #[test]
    fn a_what_shorter_than_eight_columns_stays_while_it_fits_whole() {
        let (screen, mut picker) = setup_with("what", "", &recently_what());
        let lines = render(&mut terminal(22, 6), &screen, &mut picker, &Slot::Waiting);
        let row = lines.iter().find(|row| row.starts_with("  bbb")).unwrap();
        assert!(row.contains("cache") && row.ends_with("  1m"), "{row}");
    }

    #[test]
    fn a_what_with_less_than_eight_columns_is_left_out() {
        let (screen, mut picker) = setup("ui", "");
        for (width, what) in [(42, true), (40, false)] {
            let lines = render(
                &mut terminal(width, 12),
                &screen,
                &mut picker,
                &Slot::Waiting,
            );
            let row = lines.iter().find(|row| row.starts_with("  lab-b")).unwrap();
            assert_eq!(row.contains("build box"), what, "{width}: {row}");
            assert!(row.ends_with("LAB"), "{width}: {row}");
        }
    }

    #[test_case(30; "ENV takes what HOST would")]
    #[test_case(20; "HOST is cut with an ellipsis")]
    #[test_case(12; "nothing is left of HOST")]
    fn env_outranks_host_in_a_narrow_list(width: u16) {
        let (screen, mut picker) = setup("ui", "consul");
        let lines = render(&mut terminal(width, 4), &screen, &mut picker, &ready());
        let row = lines
            .iter()
            .find(|row| row.contains("STAGING"))
            .unwrap_or(&lines[1]);
        assert!(row.ends_with("NEW STAGING") || width < 15, "{width}: {row}");
        assert!(row.chars().count() < usize::from(width), "{width}: {row}");
        if width == 20 {
            assert_eq!(row, "  ne…  NEW STAGING");
        }
    }

    #[test]
    fn a_long_what_ends_in_an_ellipsis_on_a_character_boundary() {
        let (screen, mut picker) = setup("ui", "consul");
        for (width, what) in [
            (55, "consul server 1 — mgmt"),
            (54, "consul server 1 — mg…"),
            (50, "consul server 1 …"),
            (41, "consul …"),
        ] {
            let mut terminal = terminal(width, 20);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            let row = lines
                .iter()
                .find(|row| row.starts_with("  newstg-consul1"))
                .unwrap();
            assert!(row.contains(what), "{width}: {row}");
            assert!(!row.contains('…') || what.ends_with('…'), "{width}: {row}");
        }
    }

    #[test_case(false)]
    #[test_case(true)]
    fn bold_survives_inside_the_kept_part_and_the_ellipsis_is_plain(color: bool) {
        let (screen, mut picker) = colored(color, "server");
        let mut terminal = terminal(48, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let row = lines.iter().position(|row| row.contains("…")).unwrap() as u16;
        let from = u16::try_from(lines[usize::from(row)].find("server").unwrap()).unwrap();
        let is_bold = |x| bold(&cell(&terminal, x, row));
        assert!((from..from + 6).all(is_bold), "server");
        let ellipsis = u16::try_from(lines[usize::from(row)].find('…').unwrap()).unwrap();
        assert!(!is_bold(ellipsis), "the ellipsis is not a match");
        assert!(is_bar(&cell(&terminal, ellipsis, row), color));
    }

    #[test]
    fn moving_the_selection_moves_only_the_bar() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let mut frames = Vec::new();
        // A long route, a short one and none at all: the caps never move with what the route says.
        let slots = [
            ready(),
            Slot::Waiting,
            Slot::Ready(Preview::parse("user ubuntu\nport 22\n")),
        ];
        for (at, slot) in slots.iter().enumerate() {
            let lines = render(&mut terminal, &screen, &mut picker, slot);
            assert_eq!(lines.len(), 20, "the panel does not move");
            frames.push(lines);
            if at + 1 < slots.len() {
                picker.handle(Key::Down);
            }
        }
        for (before, after) in frames.iter().zip(&frames[1..]) {
            assert_eq!(
                &before[..before.len() - 1],
                &after[..after.len() - 1],
                "every row but the footer is the same: {before:?} / {after:?}"
            );
        }
        let footers: Vec<_> = frames.iter().map(|lines| lines.last().unwrap()).collect();
        let caps = |footer: &String| footer.chars().position(|c| c == 'K');
        for (before, after) in footers.iter().zip(&footers[1..]) {
            assert_eq!(
                caps(before),
                caps(after),
                "the caps stay put whatever the route says: {before:?} / {after:?}"
            );
            assert!(
                before.ends_with("Keys ?    Connect ↵") && after.ends_with("Keys ?    Connect ↵"),
                "{before:?} / {after:?}"
            );
        }
        let (screen, mut picker) = setup("ui", "");
        picker.handle(Key::Down);
        render(&mut terminal, &screen, &mut picker, &ready());
        assert!(!is_bar_row(&terminal, 2, false) && is_bar_row(&terminal, 3, false));
    }

    #[test]
    fn typing_keeps_the_same_panel_and_the_same_rows() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        render(&mut terminal, &screen, &mut picker, &ready());
        let frame: Vec<String> = screen_lines(&terminal)
            .iter()
            .map(|row| {
                row.chars()
                    .map(|c| {
                        if "╭╮╰╯─│├┤".contains(c) {
                            c
                        } else {
                            'x'
                        }
                    })
                    .collect()
            })
            .collect();
        for c in "con".chars() {
            picker.handle(Key::Char(c));
        }
        render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(
            screen_lines(&terminal)
                .iter()
                .map(|row| {
                    row.chars()
                        .map(|c| {
                            if "╭╮╰╯─│├┤".contains(c) {
                                c
                            } else {
                                'x'
                            }
                        })
                        .collect::<String>()
                })
                .collect::<Vec<_>>(),
            frame,
            "the frame is the same with a query"
        );
    }

    #[test]
    fn the_help_sits_at_the_panels_bottom_whatever_the_filter_matches() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let mut at = Vec::new();
        for query in ["", "con", "zzzz"] {
            picker.handle(Key::Ctrl('u'));
            for c in query.chars() {
                picker.handle(Key::Char(c));
            }
            if !picker.help_open() {
                picker.handle(Key::F(1));
            }
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            at.push(
                lines
                    .iter()
                    .position(|line| line.starts_with("  Enter"))
                    .unwrap(),
            );
        }
        assert_eq!(at[0], at[1], "a query does not move the help");
        assert_eq!(at[0], at[2], "no match does not move it either");
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
        let lines = render_panel(&mut terminal(80, 24), &screen, &mut picker, &ready());
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |doc| std::fs::read_to_string(root.join(doc)).unwrap();
        for doc in ["README.md", "docs/picker.md"] {
            let text = read(doc);
            let shown = sample(&text, "~/work $ keyway");
            assert_eq!(
                shown,
                Some(lines.iter().map(String::as_str).collect()),
                "{doc}"
            );
        }

        let (screen, mut picker) = setup("ui", "con");
        let narrowed = render_panel(&mut terminal(80, 24), &screen, &mut picker, &ready());
        let text = read("docs/picker.md");
        let shown = sample(&text, "~/work $ keyway con");
        assert_eq!(
            shown,
            Some(narrowed.iter().map(String::as_str).collect()),
            "a query"
        );

        let (screen, mut picker) = setup_with("ui", "", &recently_used());
        let recent = render_panel(&mut terminal(80, 24), &screen, &mut picker, &ready());
        assert_eq!(
            sample(&text, "~/work $ kw"),
            Some(recent.iter().map(String::as_str).collect()),
            "recent hosts"
        );
    }

    #[test]
    fn env_column_hides_when_no_host_has_a_label() {
        let (screen, mut picker) = setup("flat", "");
        let mut t = terminal(40, 14);
        render(&mut t, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("flat_40x14", shown(&t));
    }

    #[test]
    fn narrowing_the_query_leaves_no_stale_rows() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let all = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            all[2..14].iter().filter(|line| !line.is_empty()).count(),
            5,
            "the five hosts, one line each"
        );

        for c in "'lab-b".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("narrowed_60x20", shown(&terminal));
        assert_eq!(lines.len(), all.len(), "the panel is the terminal's size");
        assert_eq!(
            lines[2..14].iter().filter(|line| !line.is_empty()).count(),
            1,
            "only lab-b"
        );
        let screen = screen_lines(&terminal);
        assert!(screen[19..].iter().all(String::is_empty), "{screen:#?}");
    }

    #[test]
    fn the_panel_fills_the_terminal_and_ends_in_the_footer() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        for query in ["", "consul", "consul1", "zzzz"] {
            picker.handle(Key::Ctrl('u'));
            for c in query.chars() {
                picker.handle(Key::Char(c));
            }
            render(&mut terminal, &screen, &mut picker, &ready());
            let lines = screen_lines(&terminal);
            assert_eq!(lines.len(), 24, "the terminal does not change size");
            assert!(lines[0].is_empty() && lines[1].contains('╭'), "{query:?}");
            assert!(lines[20].contains('├'), "{query:?}: {lines:#?}");
            assert!(
                lines[21].contains('↵') || lines[21].contains("esc"),
                "{query:?}: {lines:#?}"
            );
            assert!(lines[22].contains('╰'), "{query:?}: {lines:#?}");
        }
    }

    #[test]
    fn the_route_is_on_the_footers_left_and_the_keys_on_its_right() {
        let (screen, mut picker) = setup("ui", "con");
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        let footer = lines.last().unwrap();
        assert!(
            footer.starts_with("  ubuntu@10.11.212.191:22 via newstg-bastion"),
            "the route is whole beside the columns the caps keep: {footer:?}"
        );
        assert!(
            footer.ends_with("Keys F1    Connect ↵"),
            "Quit is the cap a route's room drops first: {footer:?}"
        );
        let muted: Vec<u16> = (0..74)
            .filter(|x| is_muted(&cell(&terminal, *x, 19), false))
            .collect();
        let end = u16::try_from(footer.find("newstg-bastion").unwrap() + 14).unwrap();
        assert_eq!(
            muted
                .iter()
                .filter(|x| **x < end)
                .copied()
                .collect::<Vec<_>>(),
            (2..end).collect::<Vec<_>>(),
            "the route is muted"
        );
    }

    #[test_case(false)]
    #[test_case(true)]
    fn the_route_says_where_the_selected_host_goes_in_words(color: bool) {
        let (screen, mut picker) = colored(color, "");
        let mut terminal = terminal(80, 24);
        let preview = Preview::parse(
            "remotecommand sudo -i\nproxyjump a,b\nport 2222\nhostname h\nuser ops\n",
        );
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(preview));
        assert!(
            lines[19].starts_with("  ops@h:2222 via a,b runs sudo -i"),
            "{lines:#?}"
        );
        assert!((2..33).all(|x| is_muted(&cell(&terminal, x, 19), color)));

        let bare = Preview::parse("hostname h\n");
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(bare));
        assert!(lines[19].starts_with("  h"), "{lines:#?}");
        let none = Preview::parse("user ubuntu\nport 22\n");
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(none));
        assert!(
            lines[19].trim_start().starts_with("Keys ?"),
            "no hostname, no route: {lines:#?}"
        );
        assert!(
            lines[19].ends_with("Keys ?    Connect ↵"),
            "the keys keep their columns even with no route: {:?}",
            lines[19]
        );
    }

    #[test_case(false)]
    #[test_case(true)]
    fn the_route_says_resolving_while_ssh_has_not_answered(color: bool) {
        let (screen, mut picker) = colored(color, "");
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!(lines[19].starts_with("  resolving…"), "{lines:#?}");
        assert!((2..12).all(|x| is_muted(&cell(&terminal, x, 19), color)));
        assert!(lines[19].ends_with("Keys ?    Connect ↵"), "{lines:#?}");
    }

    #[test_case(false)]
    #[test_case(true)]
    fn a_failed_route_names_why_ssh_failed_in_the_fail_role(color: bool) {
        let (screen, mut picker) = colored(color, "");
        let mut terminal = terminal(80, 24);
        let bad = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "~/.ssh/config line 3: Bad option\n\
                     ~/.ssh/config: terminating, 1 bad configuration options"
                .to_owned(),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &bad);
        assert!(
            lines[19].starts_with("  ssh -G failed: ~/.ssh/config line 3: Bad option"),
            "{lines:#?}"
        );
        let width =
            u16::try_from("  ssh -G failed: ~/.ssh/config line 3: Bad option".len()).unwrap();
        assert!((2..width).all(|x| is_fail(&cell(&terminal, x, 19), color)));
        assert!(
            !lines.concat().contains("terminating"),
            "the first line only"
        );
        let lines = render(
            &mut terminal,
            &screen,
            &mut picker,
            &Slot::Failed(ResolveError::Timeout),
        );
        assert!(
            lines[19].starts_with("  ssh -G gave no answer within 2 s"),
            "{lines:#?}"
        );
    }

    #[test]
    fn a_route_wider_than_its_room_is_cut_with_an_ellipsis() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(30, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        let footer = lines.last().unwrap();
        assert!(footer.starts_with("  ubuntu@10.11.21…"), "{footer:?}");
        assert!(
            is_muted(&cell(&terminal, 17, 11), false),
            "the cut is muted too: {footer:?}"
        );
    }

    #[test]
    fn control_characters_in_what_ssh_printed_never_reach_the_screen() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let hostile = Preview::parse("user a\x07b\nhostname c\x1b[31md\nproxyjump e\tf\n");
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Ready(hostile));
        assert!(lines[19].starts_with("  a"), "{lines:#?}");
        assert!(!lines.concat().chars().any(char::is_control), "{lines:#?}");
        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "bad \x1b[31moption\x07".to_owned(),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert!(
            lines[19].starts_with("  ssh -G failed: bad  [31moption"),
            "{lines:#?}"
        );
    }

    #[test]
    fn a_wide_terminal_draws_the_same_panel_wider() {
        let (screen, mut picker) = setup("ui", "");
        let mut wide = terminal(120, 30);
        render(&mut wide, &screen, &mut picker, &ready());
        assert_snapshot!("wide_120x30", shown(&wide));
        assert_snapshot!("wide_120x30_styles", styles(&wide));
        wide.assert_cursor(2, 0);

        let (screen, mut picker) = setup("ui", "consul");
        let mut t = terminal(100, 24);
        render(&mut t, &screen, &mut picker, &ready());
        assert_snapshot!("wide_100x24_consul", shown(&t));
    }

    #[test]
    fn a_short_terminal_is_filled_with_the_footer_on_the_last_row() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 12);
        render(&mut terminal, &screen, &mut picker, &ready());
        assert_snapshot!("short_80x12", shown(&terminal));
        terminal.assert_cursor(2, 0);
    }

    fn rows_of(lines: &[String]) -> String {
        lines
            .iter()
            .enumerate()
            .map(|(y, line)| match line {
                _ if y == 0 => '>',
                line if line.is_empty() => '_',
                line if line.starts_with('─') => '-',
                line if line.contains('↵') => 'K',
                _ => 'L',
            })
            .collect()
    }

    #[test_case(80, 24, ">_LLLLL___________-K"; "everything the terminal has")]
    #[test_case(40, 13, ">_LLLLL_____K"; "to the row")]
    #[test_case(60, 12, ">_LLLL-K"; "bordered")]
    #[test_case(60, 9, ">_LLLLL_K"; "no border below ten rows: the rows go to the picker")]
    #[test_case(60, 6, ">_LLLK"; "a shorter terminal is the same panel, shorter")]
    #[test_case(100, 5, ">LLLK"; "shorter still: the blank row goes first")]
    #[test_case(80, 4, ">LLK"; "then the gap")]
    #[test_case(80, 3, ">LK"; "then another row")]
    #[test_case(80, 2, ">L"; "then the footer")]
    #[test_case(80, 1, ">"; "the prompt alone")]
    fn a_shorter_terminal_gets_fewer_rows_of_the_same_panel(cols: u16, rows: u16, want: &str) {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(cols, rows);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(rows_of(&lines), want, "{lines:#?}");
        if let Some(first) = want.find('L') {
            let y = u16::try_from(first).unwrap();
            assert!(
                is_bar_row(&terminal, y, false),
                "the first host line is the selection"
            );
        }
    }

    #[test_case(62; "a wider terminal")]
    #[test_case(63; "an odd margin: the extra column is on the right")]
    #[test_case(80; "80 columns")]
    #[test_case(99; "99")]
    #[test_case(100; "100")]
    #[test_case(161; "the cap")]
    fn from_60_columns_the_panel_is_bordered_and_centred_with_blank_cells_around_it(width: u16) {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(width, 24);
        render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let lines = screen_lines(&terminal);
        let cols = (width - 4).min(110);
        let left = usize::from(width - cols) / 2;
        let pad = " ".repeat(left);
        let rule = "─".repeat(usize::from(cols) - 2);
        assert!(lines[0].is_empty(), "a blank row above");
        assert_eq!(lines[1], format!("{pad}╭{rule}╮"));
        assert_eq!(lines[20], format!("{pad}├{rule}┤"));
        assert_eq!(lines[22], format!("{pad}╰{rule}╯"));
        assert!(lines[23].is_empty(), "a blank row below");
        for (y, line) in lines[2..22].iter().enumerate() {
            if y + 2 == 20 {
                continue;
            }
            let chars: Vec<char> = line.chars().collect();
            assert_eq!(chars.len(), left + usize::from(cols), "row {y}: {line:?}");
            assert!(chars[..left].iter().all(|c| *c == ' '), "row {y}");
            assert_eq!(
                (chars[left], chars[left + usize::from(cols) - 1]),
                ('│', '│'),
                "row {y}"
            );
        }
        assert!(
            lines[2].contains("Search host, ENV or what") && lines[2].contains("5 hosts"),
            "{:?}",
            lines[2]
        );
        terminal.assert_cursor(2, 0);
        let inner = cols - 2;
        assert!(
            (0..inner)
                .all(|x| is_bar(&cell(&terminal, x, 2), false) == (1..inner - 1).contains(&x)),
            "the bar keeps a column at each side inside the border"
        );
    }

    #[test_case(59; "59 columns")]
    #[test_case(50; "50")]
    #[test_case(40; "40")]
    fn below_60_columns_the_panel_is_the_full_width_with_no_border(width: u16) {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(width, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines.len(), 24, "the whole terminal");
        assert!(lines[2].starts_with("  lab-a") && lines[2].ends_with("  LAB"));
        assert!(lines[23].contains('↵'));
        assert!(
            !screen_lines(&terminal).concat().contains(['─', '│', '╭']),
            "no border"
        );
        assert!((1..width - 1).all(|x| is_bar(&cell(&terminal, x, 2), false)));
        terminal.assert_cursor(2, 0);
    }

    #[test_case(false)]
    #[test_case(true)]
    fn the_border_and_the_rule_are_stroke_and_nothing_else_is_drawn_in_the_margin(color: bool) {
        let (screen, mut picker) = colored(color, "");
        let mut terminal = terminal(120, 30);
        render(&mut terminal, &screen, &mut picker, &ready());
        let at = |x, y| screen_cell(&terminal, x, y);
        let (left, right) = (5, 114);
        let edge = [
            (left, 1, "╭"),
            (right, 1, "╮"),
            (left, 28, "╰"),
            (right, 28, "╯"),
            (left, 26, "├"),
            (right, 26, "┤"),
        ];
        for (x, y, symbol) in edge {
            assert_eq!(at(x, y).symbol(), symbol);
            assert!(is_stroke(&at(x, y), color), "{symbol}");
        }
        for x in left + 1..right {
            for y in [1, 26, 28] {
                assert!(is_stroke(&at(x, y), color), "{x},{y}");
            }
        }
        for y in (2..26).chain(27..28) {
            for x in [left, right] {
                assert_eq!(at(x, y).symbol(), "│", "{x},{y}");
                assert!(is_stroke(&at(x, y), color), "{x},{y}");
            }
        }
        for y in 0..30 {
            for x in (0..left).chain(right + 1..120) {
                assert!(is_plain(&at(x, y)) && at(x, y).symbol() == " ", "{x},{y}");
            }
        }
        assert!((0..120).all(|x| is_plain(&at(x, 0)) && at(x, 0).symbol() == " "));
        assert!((0..120).all(|x| is_plain(&at(x, 29)) && at(x, 29).symbol() == " "));
    }

    #[test_case(false)]
    #[test_case(true)]
    fn matches_are_bold_in_the_colour_of_the_text_they_sit_in(color: bool) {
        let drawn = |query| {
            let (screen, mut picker) = colored(color, query);
            let mut terminal = terminal(80, 24);
            render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            terminal
        };
        let all = |t: &Block, xs: std::ops::Range<u16>, y, is: &dyn Fn(&_) -> bool| {
            xs.into_iter().all(|x| is(&cell(t, x, y)))
        };

        // HOST: lab-a is the bar, lab-b the row under it.
        let t = drawn("lab");
        assert!(
            all(&t, 2..5, 2, &|c| is_bar_match(c, color)),
            "HOST on the bar"
        );
        assert!(
            all(&t, 5..6, 2, &|c| is_bar(c, color) && !bold(c)),
            "the rest of it"
        );
        assert!(all(&t, 2..5, 3, &|c| is_match(c)), "HOST off the bar");
        assert!(all(&t, 5..7, 3, &|c| is_plain(c)), "the rest of it");

        // ENV.
        let t = drawn("LAB");
        assert!(
            all(&t, 69..72, 2, &|c| is_bar_match(c, color)),
            "ENV on the bar"
        );
        assert!(
            all(&t, 69..72, 3, &|c| is_muted_match(c, color)),
            "ENV off the bar"
        );

        // WHAT: an `o` on the bar is a bar match, off it a muted one.
        let mut t = drawn("o");
        let bar_y = (2..17).find(|y| is_bar_row(&t, *y, color)).unwrap();
        let o = |y| (0..t.width()).find(|x| cell(&t, *x, y).symbol() == "o");
        let on = o(bar_y).unwrap();
        assert!(is_bar_match(&cell(&t, on, bar_y), color), "WHAT on the bar");
        let off = (2..17)
            .filter(|y| *y != bar_y)
            .find_map(|y| {
                // WHAT starts two columns past the widest host name, at 18 here.
                (18..t.width())
                    .find(|x| cell(&t, *x, y).symbol() == "o")
                    .map(|x| (x, y))
            })
            .unwrap();
        assert!(
            is_muted_match(&cell(&t, off.0, off.1), color),
            "WHAT off the bar"
        );
        assert!(
            t.backend()
                .buffer()
                .content()
                .iter()
                .all(|c| !(bold(c) && dim(c))),
            "a match is never dim"
        );
        t.assert_cursor(3, 0);
    }

    #[test]
    fn the_screen_uses_the_grey_ramp_and_ansi_4_only_and_none_without_colour() {
        for (query, width, height) in [("", 120, 30), ("consul", 80, 24), ("zzzz", 100, 12)] {
            let (screen, mut picker) = setup("ui", query);
            picker.handle(Key::F(1));
            let mut plain = terminal(width, height);
            render(&mut plain, &screen, &mut picker, &ready());
            assert!(
                plain
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset),
                "{query:?} at {width}x{height}"
            );

            let (screen, mut picker) = setup_colored("ui", query);
            picker.handle(Key::F(1));
            let mut colored = terminal(width, height);
            render(&mut colored, &screen, &mut picker, &ready());
            let ramp = [
                Color::Reset,
                Color::Red,
                Color::Blue,
                Color::Indexed(236),
                Color::Indexed(245),
                Color::Indexed(250),
                Color::Indexed(255),
            ];
            assert!(
                colored
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .all(|cell| ramp.contains(&cell.fg) && ramp.contains(&cell.bg)),
                "{query:?} at {width}x{height}: only the grey ramp and ANSI 4"
            );
        }
    }

    #[test]
    fn the_only_line_drawing_is_the_border_and_its_rule() {
        for (width, height) in [(80, 24), (80, 12), (99, 20), (100, 24), (120, 30), (80, 10)] {
            let (screen, mut picker) = setup("ui", "");
            let lines = render(&mut terminal(width, height), &screen, &mut picker, &ready());
            let rules = lines.iter().filter(|line| line.starts_with('─')).count();
            assert_eq!(rules, 1, "{width}x{height}: one rule: {lines:#?}");
            assert!(
                !lines.concat().contains(['│', '┌', '┐', '┘', '▌', '└']),
                "{width}x{height}: {lines:#?}"
            );
        }
        for (width, height) in [(60, 7), (59, 24), (80, 9)] {
            let (screen, mut picker) = setup("ui", "");
            let lines = render(&mut terminal(width, height), &screen, &mut picker, &ready());
            assert!(
                !lines.concat().contains(['─', '│', '┌', '┐', '┘', '▌', '└']),
                "no border: {width}x{height}: {lines:#?}"
            );
        }
    }

    #[test]
    fn small_60x14_terminal_draws_what_fits() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 14);
        let failed = Slot::Failed(ResolveError::Timeout);
        render(&mut terminal, &screen, &mut picker, &failed);
        assert_snapshot!("small_60x14_failed", shown(&terminal));
    }

    #[test]
    fn small_50x14_terminal_draws_what_fits() {
        let (screen, mut picker) = setup("ui", "con");
        let mut t = terminal(50, 14);
        render(&mut t, &screen, &mut picker, &ready());
        assert_snapshot!("con_50x14", shown(&t));
    }

    #[test_case(false)]
    #[test_case(true)]
    fn the_footer_is_muted_labels_before_keycaps_at_the_right(color: bool) {
        let (screen, mut picker) = colored(color, "");
        let mut small = terminal(80, 7);
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        let row = lines[6].clone();
        assert!(row.ends_with(KEYS), "{row:?}");
        assert!(
            row.starts_with("  resolving…"),
            "the route at the left: {row:?}"
        );
        let at = |x| cell(&small, x, 6);
        let from = u16::try_from(row.chars().count() - KEYS.chars().count()).unwrap();
        assert!(is_plain(&at(0)) && is_plain(&at(79)), "the margins");
        let keys: Vec<_> = (from..79).map(|x| (x, at(x))).collect();
        let cap = |label: &str, key: &str| {
            let text: String = keys.iter().map(|(_, cell)| cell.symbol()).collect();
            let at = text
                .find(&format!("{label} {key} "))
                .unwrap_or_else(|| panic!("{text:?}"));
            let first = from + u16::try_from(text[..at].chars().count()).unwrap();
            let label_len = u16::try_from(label.chars().count()).unwrap();
            let key_len = u16::try_from(key.chars().count()).unwrap();
            (first, label_len, key_len)
        };
        for (label, key) in [("Keys", "?"), ("Quit", "esc"), ("Connect", "↵")] {
            let (first, label_len, key_len) = cap(label, key);
            assert!(
                (first..first + label_len).all(|x| is_muted(&at(x), color)),
                "{label} is muted"
            );
            assert!(
                (first + label_len..first + label_len + key_len + 2)
                    .all(|x| is_keycap(&at(x), color)),
                "{key} is a keycap with a space either side"
            );
        }
        assert!(
            is_keycap(&at(78), color),
            "the last cap ends at the bar's edge"
        );

        picker.handle(Key::Char('c'));
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        assert!(
            lines[6].ends_with("Keys F1    Quit esc    Connect ↵"),
            "? is a character once there is a query: {:?}",
            lines[6]
        );

        picker.handle(Key::F(1));
        let lines = render(&mut terminal(80, 24), &screen, &mut picker, &Slot::Waiting);
        assert!(
            lines
                .last()
                .is_some_and(|row| row.ends_with("Close esc    Connect ↵")),
            "Esc closes the help first, and the help key is not offered: {lines:#?}"
        );
    }

    #[test_case(false)]
    #[test_case(true)]
    fn zero_matches_say_no_host_matches_on_the_first_row_and_the_footer_offers_only_quit(
        color: bool,
    ) {
        let (screen, mut picker) = colored(color, "zzzz");
        let mut wide = terminal(99, 20);
        let lines = render(&mut wide, &screen, &mut picker, &ready());
        assert_eq!(lines[2], "  No host matches \"zzzz\"");
        assert!(
            (2..24).all(|x| is_muted(&cell(&wide, x, 2), color)),
            "muted"
        );
        assert!((24..56).all(|x| is_plain(&cell(&wide, x, 2))));
        assert!((0..56).all(|x| !reversed(&cell(&wide, x, 2))));
        assert_eq!(
            lines[2..].iter().filter(|line| !line.is_empty()).count(),
            3,
            "the no-match line, the rule and the footer"
        );
        assert!(
            lines
                .last()
                .is_some_and(|row| row.ends_with("Keys F1    Quit esc")),
            "no connect: {:?}",
            lines.last()
        );
    }

    #[test]
    fn the_list_scrolls_whole_hosts_and_keeps_the_selection_in_view() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(70, 8);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!(lines[2].starts_with("  lab-a"), "{lines:#?}");
        assert!(lines[6].starts_with("  newstg-bastion"), "{lines:#?}");
        assert!(lines[7].contains('↵'));
        assert_snapshot!("scroll_top_70x8", shown(&terminal));

        for _ in 0..4 {
            picker.handle(Key::Down);
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("scroll_end_70x8", shown(&terminal));
        assert!(lines[2].starts_with("  lab-a"), "{lines:#?}");
        assert!(lines[6].starts_with("  newstg-bastion"), "{lines:#?}");
        assert!(is_bar_row(&terminal, 6, false));
        assert!(!is_bar_row(&terminal, 2, false));

        let mut tall = self::terminal(70, 8);
        let (screen, mut picker) = setup("ui", "'lab");
        let lines = render(&mut tall, &screen, &mut picker, &Slot::Waiting);
        assert!(lines[2].starts_with("  lab-b") && lines[3].starts_with("  lab-a"));
        assert!(lines[7].contains("Connect ↵"), "the block keeps its height");
    }

    #[test]
    fn a_block_with_no_room_for_the_selected_host_says_it_on_the_prompt_row() {
        let (screen, mut picker) = setup("ui", "");
        let mut one = terminal(60, 1);
        let lines = render(&mut one, &screen, &mut picker, &ready());
        assert_eq!(lines, [format!("{PROMPT:<53}lab-a")]);
        assert_eq!(cell(&one, 53, 0).symbol(), "l", "the host is the thing");
        one.assert_cursor(2, 0);

        let (screen_in_colour, mut coloured) = setup_colored("ui", "");
        let mut one_in_colour = terminal(60, 1);
        render(
            &mut one_in_colour,
            &screen_in_colour,
            &mut coloured,
            &ready(),
        );
        let at = |x| cell(&one_in_colour, x, 0);
        assert_eq!(at(53).symbol(), "l");
        assert!(
            (1..59).all(|x| !is_plain(&at(x))) && is_plain(&at(0)) && is_plain(&at(59)),
            "the host is on the strip, margin to margin"
        );
        assert!(
            (53..58).all(|x| is_typed(&at(x), true)),
            "the host is typed text"
        );
        assert!(
            is_field(&at(58), true),
            "and the strip has a column after it"
        );

        picker.handle(Key::Down);
        let lines = render(&mut one, &screen, &mut picker, &ready());
        assert!(
            lines[0].ends_with("lab-b"),
            "it follows the selection: {lines:?}"
        );

        for c in "zzzz".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut one, &screen, &mut picker, &ready());
        assert_eq!(
            lines,
            [format!("{:<53}0 / 5", "  zzzz")],
            "no selection, the count is back"
        );
    }

    #[test]
    fn tiny_terminals_never_panic() {
        let (screen, mut picker) = setup("ui", "consul");
        let sizes = (0..=60)
            .map(|width| (width, 14))
            .chain([(99, 20), (100, 20), (101, 20), (140, 20)])
            .flat_map(|(width, most)| (0..=most).map(move |height| (width, height)));
        for (width, height) in sizes {
            let mut terminal = terminal(width, height);
            let failed = Slot::Failed(ResolveError::Timeout);
            render(&mut terminal, &screen, &mut picker, &failed);
            let lines = render(&mut terminal, &screen, &mut picker, &ready());
            if width >= 10 && height >= 2 {
                assert!(
                    lines[0].starts_with("  consul"),
                    "{width}x{height}: {lines:#?}"
                );
            }
        }
        let lines = render(&mut terminal(30, 1), &screen, &mut picker, &Slot::Waiting);
        assert!(lines[0].starts_with("  consul"), "the prompt still draws");
    }
}
