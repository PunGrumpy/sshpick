//! The block is the whole screen: rows the hosts do not need are list rows left blank, so the
//! route and the hint stay at the bottom. A shorter terminal gets fewer rows of the same block,
//! never another layout. A wider one moves the route out of its row into a panel beside the list.

/// Prompt, header and hint: the rows that are not hosts, without the route row.
const CHROME: u16 = 3;
/// The list keeps 60 columns beside a panel of two fifths of the width.
const PANEL_FROM: u16 = 100;
/// The host, then one row for each piece of a route, so that the panel is never cut.
const PANEL_ROWS: u16 = 6;
/// With the help open the list gives up rows down to this many before the help is cut.
const LIST_WITH_HELP: u16 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Space {
    pub(super) cols: u16,
    pub(super) rows: u16,
}

impl Space {
    /// Some pty wrappers report 0x0. That is taken as 80x24 rather than a reason not to open.
    pub(super) fn reported(cols: u16, rows: u16) -> Space {
        if cols == 0 || rows == 0 {
            Space { cols: 80, rows: 24 }
        } else {
            Space { cols, rows }
        }
    }
}

/// The block's rows, top to bottom: the prompt (always), the header, `list` host rows, the route,
/// `help` rows of key help and the hint. With `panel`, the route sits beside the header and the
/// list instead of in a row of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Plan {
    pub(super) cols: u16,
    pub(super) header: bool,
    pub(super) list: u16,
    pub(super) route: bool,
    pub(super) panel: bool,
    pub(super) help: u16,
    pub(super) hint: bool,
}

impl Plan {
    pub(super) fn height(self) -> u16 {
        1 + self.beside() + u16::from(self.route) + self.help + u16::from(self.hint)
    }

    /// The header and the host rows, which the panel sits beside.
    pub(super) fn beside(self) -> u16 {
        u16::from(self.header) + self.list
    }

    pub(super) fn panel_cols(self) -> u16 {
        if self.panel { self.cols / 5 * 2 } else { 0 }
    }
}

/// What gets a row as the block gets taller, after the prompt.
const LADDER: [Part; 6] = [
    Part::List,
    Part::Hint,
    Part::List,
    Part::List,
    Part::Route,
    Part::Header,
];

#[derive(Clone, Copy)]
enum Part {
    List,
    Hint,
    Route,
    Header,
}

/// `hosts` is every host, not the matches: the rows that are not theirs are left blank and every
/// short terminal drops the same rows whatever the query. `help` is the rows the open key help
/// wants, 0 when it is closed.
pub(super) fn plan(space: Space, hosts: usize, help: u16) -> Plan {
    let hosts = u16::try_from(hosts).unwrap_or(u16::MAX).max(1);
    if space.cols >= PANEL_FROM {
        let wide = layout(space, hosts, help, true);
        if wide.beside() >= PANEL_ROWS {
            return wide;
        }
    }
    layout(space, hosts, help, false)
}

fn layout(space: Space, hosts: u16, help: u16, panel: bool) -> Plan {
    let chrome = CHROME + u16::from(!panel);
    let wanted = chrome.saturating_add(hosts).saturating_add(help);
    let screen = space.rows.max(1);
    let height = wanted.min(screen);
    let whole = Plan {
        cols: space.cols,
        header: true,
        list: hosts,
        route: !panel,
        panel,
        help,
        hint: true,
    };
    let mut plan = if height == wanted {
        whole
    } else if help > 0 && height > chrome + LIST_WITH_HELP {
        let list = hosts
            .min(height.saturating_sub(chrome + help))
            .max(hosts.min(LIST_WITH_HELP));
        Plan {
            list,
            help: height - chrome - list,
            ..whole
        }
    } else {
        squeeze(space.cols, height, hosts, panel)
    };
    plan.list += screen - plan.height();
    plan
}

/// A host row is skipped once every host has one, so a short config never gets a blank row ahead
/// of the route. A panel costs no row, so its rung is skipped too.
fn squeeze(cols: u16, height: u16, hosts: u16, panel: bool) -> Plan {
    let mut plan = Plan {
        cols,
        header: false,
        list: 0,
        route: false,
        panel,
        help: 0,
        hint: false,
    };
    let mut left = height.saturating_sub(1);
    for part in LADDER {
        if left == 0 {
            break;
        }
        match part {
            Part::List if plan.list >= hosts => continue,
            Part::List => plan.list += 1,
            Part::Hint => plan.hint = true,
            Part::Route if panel => continue,
            Part::Route => plan.route = true,
            Part::Header => plan.header = true,
        }
        left -= 1;
    }
    plan.list += left.min(hosts - plan.list);
    plan
}

#[cfg(test)]
mod tests {
    use sshpick_core::BINDINGS;
    use test_case::test_case;

    use super::*;

    fn keys() -> u16 {
        u16::try_from(BINDINGS.len() + 1).unwrap()
    }

    fn shape(plan: Plan) -> (u16, bool, u16, bool, u16, bool) {
        (
            plan.height(),
            plan.header,
            plan.list,
            plan.route,
            plan.help,
            plan.hint,
        )
    }

    // The ladder in docs/picker.md.
    //          cols rows hosts    height header list route help hint
    #[test_case(80, 24, 5, (24, true, 20, true, 0, true); "80x24, five hosts: the list takes the rest")]
    #[test_case(80, 24, 1, (24, true, 20, true, 0, true); "a short config is still the whole screen")]
    #[test_case(80, 24, 29, (24, true, 20, true, 0, true); "29 hosts: the list scrolls")]
    #[test_case(60, 12, 5, (12, true, 8, true, 0, true); "60x12")]
    #[test_case(40, 8, 5, (8, true, 4, true, 0, true); "40x8")]
    #[test_case(80, 7, 5, (7, true, 3, true, 0, true); "the shortest with every row")]
    #[test_case(80, 6, 5, (6, false, 3, true, 0, true); "the header goes first")]
    #[test_case(80, 5, 5, (5, false, 3, false, 0, true); "then the route")]
    #[test_case(80, 4, 5, (4, false, 2, false, 0, true); "then a third host")]
    #[test_case(80, 4, 1, (4, false, 1, true, 0, true); "one host: the route takes the second host's row")]
    #[test_case(80, 3, 5, (3, false, 1, false, 0, true); "enter and esc outrank a second host")]
    #[test_case(80, 2, 5, (2, false, 1, false, 0, false); "the hint is the next to go")]
    #[test_case(80, 1, 5, (1, false, 0, false, 0, false); "80x1: the prompt alone")]
    #[test_case(80, 0, 5, (1, false, 0, false, 0, false); "80x0: the prompt alone")]
    fn the_size_picks_how_much_of_the_block_there_is(
        cols: u16,
        rows: u16,
        hosts: usize,
        want: (u16, bool, u16, bool, u16, bool),
    ) {
        assert_eq!(shape(plan(Space { cols, rows }, hosts, 0)), want);
    }

    //          rows    height header list route help hint
    #[test_case(24, (24, true, 9, true, 11, true); "the help takes blank rows first")]
    #[test_case(20, (20, true, 5, true, 11, true); "the shortest terminal that takes all of it")]
    #[test_case(19, (19, true, 4, true, 11, true); "the list gives up a row before the help does")]
    #[test_case(18, (18, true, 3, true, 11, true); "down to three rows")]
    #[test_case(17, (17, true, 3, true, 10, true); "then the help is cut")]
    #[test_case(12, (12, true, 3, true, 5, true); "80x12")]
    #[test_case(8, (8, true, 3, true, 1, true); "one row of help")]
    #[test_case(7, (7, true, 3, true, 0, true); "no room for help: the block is as if it were closed")]
    #[test_case(6, (6, false, 3, true, 0, true); "a short terminal keeps the closed ladder")]
    fn the_help_takes_rows_from_the_blank_rows_then_from_the_list(
        rows: u16,
        want: (u16, bool, u16, bool, u16, bool),
    ) {
        assert_eq!(shape(plan(Space { cols: 80, rows }, 5, keys())), want);
    }

    // The header and the list give the panel its six rows, or the route keeps its row.
    //          cols rows hosts    height header list route panel
    #[test_case(100, 24, 5, (24, true, 21, false, true); "100x24: the route needs no row")]
    #[test_case(99, 24, 5, (24, true, 20, true, false); "one column short: the route row")]
    #[test_case(120, 30, 29, (30, true, 27, false, true); "29 hosts: the route's row goes to the list")]
    #[test_case(120, 24, 1, (24, true, 21, false, true); "one host beside blank rows")]
    #[test_case(120, 8, 5, (8, true, 5, false, true); "the shortest that holds it")]
    #[test_case(120, 7, 5, (7, true, 3, true, false); "a row shorter: the route row")]
    fn from_100_columns_the_route_is_a_panel_beside_the_list(
        cols: u16,
        rows: u16,
        hosts: usize,
        want: (u16, bool, u16, bool, bool),
    ) {
        let p = plan(Space { cols, rows }, hosts, 0);
        assert_eq!((p.height(), p.header, p.list, p.route, p.panel), want);
    }

    //          rows    height list help panel
    #[test_case(24, (24, 10, 11, true); "the help keeps the panel")]
    #[test_case(19, (19, 5, 11, true); "the shortest terminal that takes all of it")]
    #[test_case(18, (18, 3, 11, false); "a list cut to three is too short for it")]
    fn the_help_keeps_the_panel_while_the_list_has_its_rows(
        rows: u16,
        want: (u16, u16, u16, bool),
    ) {
        let p = plan(Space { cols: 120, rows }, 5, keys());
        assert_eq!((p.height(), p.list, p.help, p.panel), want);
    }

    #[test]
    fn the_panel_takes_two_fifths_of_the_width() {
        let wide = |cols| plan(Space { cols, rows: 24 }, 5, 0).panel_cols();
        assert_eq!([wide(99), wide(100), wide(120), wide(200)], [0, 40, 48, 80]);
    }

    #[test]
    fn whatever_the_size_the_block_is_the_screen_and_keeps_what_matters() {
        for (cols, rows) in [1, 80, 120]
            .into_iter()
            .flat_map(|c| (0..=60).map(move |r| (c, r)))
        {
            for hosts in [0, 1, 2, 5, 29, 500] {
                for help in [0, keys()] {
                    let p = plan(Space { cols, rows }, hosts, help);
                    let height = p.height();
                    assert_eq!(height, rows.max(1), "{cols}x{rows}: {p:?}");
                    if height >= 2 {
                        assert!(p.list >= 1, "a block with room shows a host: {p:?}");
                    }
                    if height >= 3 {
                        assert!(p.hint, "enter and esc stay in sight: {p:?}");
                    }
                    assert!(!(p.route && p.panel), "the route is drawn once: {p:?}");
                    if p.panel {
                        assert!(p.beside() >= PANEL_ROWS, "never a cut panel: {p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_terminal_that_reports_no_size_is_taken_as_80x24() {
        let usual = Space { cols: 80, rows: 24 };
        assert_eq!(Space::reported(0, 0), usual);
        assert_eq!(Space::reported(0, 30), usual);
        assert_eq!(Space::reported(100, 0), usual);
        assert_eq!(Space::reported(30, 2), Space { cols: 30, rows: 2 });
    }
}
