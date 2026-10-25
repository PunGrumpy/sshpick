//! On two rows or more the block never takes the whole screen, so the command that started it
//! stays in sight. A shorter terminal gets fewer rows of the same block, never another layout.

/// Prompt, header, route and hint: the rows that are not hosts.
const CHROME: u16 = 4;
const FLOOR: u16 = 8;
/// With the help open the list gives up rows down to this many before the help is cut.
const LIST_WITH_HELP: u16 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Space {
    pub(super) cols: u16,
    pub(super) rows: u16,
}

impl Space {
    /// Some pty wrappers report 0x0. Relative moves work at any size, so that is taken as 80x24
    /// rather than a reason not to open.
    pub(super) fn reported(cols: u16, rows: u16) -> Space {
        if cols == 0 || rows == 0 {
            Space { cols: 80, rows: 24 }
        } else {
            Space { cols, rows }
        }
    }
}

/// The block's rows, top to bottom: the prompt (always), the header, `list` host rows, the route,
/// `help` rows of key help and the hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Plan {
    pub(super) cols: u16,
    pub(super) header: bool,
    pub(super) list: u16,
    pub(super) route: bool,
    pub(super) help: u16,
    pub(super) hint: bool,
}

impl Plan {
    pub(super) fn height(self) -> u16 {
        1 + u16::from(self.header)
            + self.list
            + u16::from(self.route)
            + self.help
            + u16::from(self.hint)
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

/// `hosts` is every host, not the matches, so the block keeps its height while the query changes
/// and Backspace never scrolls the terminal. `help` is the rows the open key help wants, 0 when it
/// is closed.
pub(super) fn plan(space: Space, hosts: usize, help: u16) -> Plan {
    let hosts = u16::try_from(hosts).unwrap_or(u16::MAX).max(1);
    let wanted = CHROME.saturating_add(hosts).saturating_add(help);
    let height = wanted.min(cap(space.rows, help > 0));
    if height == wanted {
        return Plan {
            cols: space.cols,
            header: true,
            list: hosts,
            route: true,
            help,
            hint: true,
        };
    }
    if help > 0 && height > CHROME + LIST_WITH_HELP {
        let list = hosts
            .min(height.saturating_sub(CHROME + help))
            .max(hosts.min(LIST_WITH_HELP));
        return Plan {
            cols: space.cols,
            header: true,
            list,
            route: true,
            help: height - CHROME - list,
            hint: true,
        };
    }
    squeeze(space.cols, height, hosts)
}

fn cap(rows: u16, help_open: bool) -> u16 {
    let most = rows.saturating_sub(1).max(1);
    if help_open {
        most
    } else {
        most.min((rows / 2).max(FLOOR))
    }
}

/// A host row is skipped once every host has one, so a short config never gets a blank row ahead
/// of the route.
fn squeeze(cols: u16, height: u16, hosts: u16) -> Plan {
    let mut plan = Plan {
        cols,
        header: false,
        list: 0,
        route: false,
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
    #[test_case(80, 24, 5, (9, true, 5, true, 0, true); "80x24, five hosts: everything the hosts need")]
    #[test_case(80, 24, 1, (5, true, 1, true, 0, true); "a short config is a short block")]
    #[test_case(80, 24, 29, (12, true, 8, true, 0, true); "29 hosts: half the terminal")]
    #[test_case(120, 30, 29, (15, true, 11, true, 0, true); "120x30: half, and the width changes nothing")]
    #[test_case(60, 12, 5, (8, true, 4, true, 0, true); "60x12: the floor of 8 rows")]
    #[test_case(40, 8, 5, (7, true, 3, true, 0, true); "40x8: all but one row")]
    #[test_case(80, 7, 5, (6, false, 3, true, 0, true); "the header goes first")]
    #[test_case(80, 6, 5, (5, false, 3, false, 0, true); "then the route")]
    #[test_case(80, 5, 5, (4, false, 2, false, 0, true); "then a third host")]
    #[test_case(80, 5, 1, (4, false, 1, true, 0, true); "one host: the route takes the second host's row")]
    #[test_case(80, 4, 5, (3, false, 1, false, 0, true); "enter and esc outrank a second host")]
    #[test_case(80, 3, 5, (2, false, 1, false, 0, false); "the hint is the next to go")]
    #[test_case(80, 2, 5, (1, false, 0, false, 0, false); "80x2: the prompt alone")]
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
    #[test_case(24, (20, true, 5, true, 11, true); "the help grows the block and the list keeps its rows")]
    #[test_case(21, (20, true, 5, true, 11, true); "the shortest terminal that takes all of it")]
    #[test_case(20, (19, true, 4, true, 11, true); "the list gives up a row before the help does")]
    #[test_case(19, (18, true, 3, true, 11, true); "down to three rows")]
    #[test_case(18, (17, true, 3, true, 10, true); "then the help is cut")]
    #[test_case(12, (11, true, 3, true, 4, true); "80x12")]
    #[test_case(9, (8, true, 3, true, 1, true); "one row of help")]
    #[test_case(8, (7, true, 3, true, 0, true); "no room for help: the block is as if it were closed")]
    #[test_case(7, (6, false, 3, true, 0, true); "a short terminal keeps the closed ladder")]
    fn the_help_takes_rows_from_the_terminal_then_from_the_list(
        rows: u16,
        want: (u16, bool, u16, bool, u16, bool),
    ) {
        assert_eq!(shape(plan(Space { cols: 80, rows }, 5, keys())), want);
    }

    #[test]
    fn whatever_the_size_the_block_fits_and_keeps_what_matters() {
        for rows in 0..=60 {
            for hosts in [0, 1, 2, 5, 29, 500] {
                for help in [0, keys()] {
                    let p = plan(Space { cols: 80, rows }, hosts, help);
                    let height = p.height();
                    assert!(height >= 1, "rows {rows}");
                    if rows >= 2 {
                        assert!(
                            height < rows,
                            "{rows} rows, {hosts} hosts, help {help}: {p:?}"
                        );
                    }
                    if height >= 2 {
                        assert!(p.list >= 1, "a block with room shows a host: {p:?}");
                    }
                    if height >= 3 {
                        assert!(p.hint, "enter and esc stay in sight: {p:?}");
                    }
                    assert!(
                        usize::from(p.list) <= hosts.max(1),
                        "never more list rows than hosts: {p:?}"
                    );
                    if help == 0 {
                        assert!(height <= (rows / 2).max(FLOOR), "at most half: {p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_taller_terminal_never_shrinks_the_block() {
        for hosts in [1, 5, 29, 500] {
            for help in [0, keys()] {
                let mut last = 0;
                for rows in 0..=80 {
                    let height = plan(Space { cols: 80, rows }, hosts, help).height();
                    assert!(
                        height >= last,
                        "{hosts} hosts: {rows} rows gave {height} after {last}"
                    );
                    last = height;
                }
            }
        }
    }

    #[test]
    fn the_width_never_changes_the_height() {
        for cols in [1, 10, 40, 80, 100, 120, 400] {
            assert_eq!(plan(Space { cols, rows: 24 }, 29, 0).height(), 12);
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
