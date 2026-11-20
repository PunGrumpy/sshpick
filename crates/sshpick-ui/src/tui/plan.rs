//! The panel is a fixed box nearly the whole terminal: the prompt, a blank row, the list, the key
//! help while it is open, and the footer last, under a rule when there is a border. A terminal
//! with fewer rows gets fewer rows of the same panel, never another layout. Every host is one
//! line, and the list fills every row the other parts do not take.

use sshpick_core::{Panel, panel};

/// Prompt, the blank row under it and the hint: the rows that are not hosts or help.
const CHROME: u16 = 3;
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

/// The panel's rows, top to bottom: the prompt (always), a blank row, `list` lines for hosts,
/// `help` rows of key help and the footer. A border adds its own rows around them, which are not
/// counted here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Plan {
    pub(super) space: Space,
    pub(super) panel: Panel,
    pub(super) gap: bool,
    pub(super) list: u16,
    pub(super) help: u16,
    pub(super) hint: bool,
}

impl Plan {
    #[cfg(test)]
    pub(super) fn height(self) -> u16 {
        1 + u16::from(self.gap) + self.list + self.help + u16::from(self.hint)
    }
}

/// What gets a row as the block gets taller, after the prompt.
const LADDER: [Part; 5] = [Part::List, Part::Hint, Part::List, Part::List, Part::Gap];

#[derive(Clone, Copy)]
enum Part {
    List,
    Hint,
    Gap,
}

/// `help` is the rows the open key help wants, 0 when it is closed. The help's rows come out of
/// the list alone, so the section sits at the bottom of the panel whatever the filter matches.
pub(super) fn plan(space: Space, help: u16) -> Plan {
    let panel = panel(space.cols, space.rows);
    let height = panel.inner_rows();
    let whole = Plan {
        space,
        panel,
        gap: true,
        list: 0,
        help,
        hint: true,
    };
    if help == 0 {
        squeeze(whole, height)
    } else if height > CHROME + LIST_WITH_HELP {
        let list = height.saturating_sub(CHROME + help).max(LIST_WITH_HELP);
        Plan {
            list,
            help: height - CHROME - list,
            ..whole
        }
    } else {
        plan(space, 0)
    }
}

/// After the prompt, the ladder hands out the rows there are; the list takes the rest.
fn squeeze(whole: Plan, height: u16) -> Plan {
    let mut plan = Plan {
        gap: false,
        list: 0,
        help: 0,
        hint: false,
        ..whole
    };
    let mut left = height.saturating_sub(1);
    for part in LADDER {
        if left == 0 {
            break;
        }
        match part {
            Part::List => plan.list += 1,
            Part::Hint => plan.hint = true,
            Part::Gap => plan.gap = true,
        }
        left -= 1;
    }
    plan.list += left;
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

    fn shape(plan: Plan) -> (u16, bool, u16, u16, bool) {
        (plan.height(), plan.gap, plan.list, plan.help, plan.hint)
    }

    // 60 columns have a border; every row but the border's five is the picker's.
    //          cols rows height gap list help hint
    #[test_case(60, 24, (19, true, 16, 0, true); "the list fills the panel")]
    #[test_case(60, 24, (19, true, 16, 0, true); "one host: the same panel")]
    #[test_case(60, 14, (9, true, 6, 0, true); "to the row")]
    #[test_case(40, 8, (8, true, 5, 0, true); "40x8: no border")]
    #[test_case(60, 8, (8, true, 5, 0, true); "60x8")]
    #[test_case(60, 6, (6, true, 3, 0, true); "to the row")]
    #[test_case(60, 5, (5, false, 3, 0, true); "the blank row under the prompt goes first")]
    #[test_case(60, 3, (3, false, 1, 0, true); "enter and esc outrank a second host")]
    #[test_case(60, 2, (2, false, 1, 0, false); "the hint is the next to go")]
    #[test_case(60, 1, (1, false, 0, 0, false); "60x1: the prompt alone")]
    #[test_case(60, 0, (1, false, 0, 0, false); "60x0: the prompt alone")]
    //          cols rows height gap list help hint
    #[test_case(80, 24, (19, true, 16, 0, true); "bordered")]
    #[test_case(120, 30, (25, true, 22, 0, true); "a wider terminal: the panel is capped")]
    #[test_case(80, 10, (5, false, 3, 0, true); "ten rows keep the border")]
    #[test_case(80, 9, (9, true, 6, 0, true); "nine rows: no border, so the rows go to the picker")]
    fn the_size_picks_how_much_of_the_panel_there_is(
        cols: u16,
        rows: u16,
        want: (u16, bool, u16, u16, bool),
    ) {
        assert_eq!(shape(plan(Space { cols, rows }, 0)), want);
    }

    // 60xN has N-5 rows inside the border from ten rows up, and every row below that.
    //          rows   height gap list help hint
    #[test_case(60, 24, (19, true, 5, 11, true); "the list gives up rows for the help")]
    #[test_case(60, 20, (15, true, 3, 9, true); "down to three rows")]
    #[test_case(60, 19, (14, true, 3, 8, true); "then the help is cut")]
    #[test_case(60, 12, (7, true, 3, 1, true); "60x12")]
    #[test_case(60, 8, (8, true, 3, 2, true); "two rows of help")]
    #[test_case(80, 24, (19, true, 5, 11, true); "bordered")]
    #[test_case(80, 12, (7, true, 3, 1, true); "bordered, the help is cut")]
    fn the_help_takes_rows_from_the_list_and_is_then_cut(
        cols: u16,
        rows: u16,
        want: (u16, bool, u16, u16, bool),
    ) {
        assert_eq!(shape(plan(Space { cols, rows }, keys())), want);
    }

    #[test]
    fn whatever_the_size_the_panel_is_inside_the_terminal_and_keeps_what_matters() {
        for (cols, rows) in [1, 59, 60, 80, 120]
            .into_iter()
            .flat_map(|c| (0..=60).map(move |r| (c, r)))
        {
            for help in [0, keys()] {
                let p = plan(Space { cols, rows }, help);
                let height = p.height();
                assert_eq!(height, p.panel.inner_rows(), "{cols}x{rows}: {p:?}");
                assert!(
                    p.panel.top + p.panel.rows <= rows.max(1),
                    "{cols}x{rows}: {p:?}"
                );
                if height >= 2 {
                    assert!(p.list >= 1, "a panel with room shows a host: {p:?}");
                }
                if height >= 3 {
                    assert!(p.hint, "enter and esc stay in sight: {p:?}");
                }
                if p.panel.border && height >= 6 {
                    assert!(p.hint && p.gap, "the border keeps every row: {p:?}");
                }
            }
        }
    }

    #[test]
    fn the_panel_size_never_depends_on_what_is_open() {
        for (cols, rows) in [(80, 24), (120, 30), (50, 20)] {
            let space = Space { cols, rows };
            let closed = plan(space, 0);
            let open = plan(space, keys());
            assert_eq!(closed.panel, open.panel, "{cols}x{rows}");
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
