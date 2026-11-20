//! The footer's keys. The row keeps columns for the route at its left — [`ROUTE_RESERVE`] of them
//! beside the full rung, [`ROUTE_RESERVE_MIN`] beside the shorter one — so the caps never move
//! with the selection. `Quit esc` is the first cap a narrow row loses; the narrowest rung is the
//! keys of what is left, without their labels. `?` is offered only while the filter is empty; it
//! is a filter character otherwise.

use crate::picker::View;

/// Columns between one cap and the next.
pub const HINT_GAP: usize = 3;
/// The columns the route's side keeps beside the full rung, its indent and the gap before the
/// keys among them.
pub const ROUTE_RESERVE: usize = 45;
/// The columns the route's side keeps beside the short rung, so `Keys` and `Connect` keep their
/// labels on a narrow row.
const ROUTE_RESERVE_MIN: usize = 24;
/// A key sits in a keycap, one space either side.
const CAP_PADDING: usize = 2;

/// A label, then its key in a keycap. The keycap's left space is all that parts them, so a cap
/// reads `Quit esc` and is the label, the key and two more columns wide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cap {
    /// Empty on the narrowest rung, which is the keys alone.
    pub label: &'static str,
    pub key: &'static str,
}

impl Cap {
    pub fn width(self) -> usize {
        width(self.label) + width(self.key) + CAP_PADDING
    }
}

/// What the footer says, left to right, at its right end, each cap as wide as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    pub caps: Vec<Cap>,
}

/// `room` is the columns of the row, margins left out. The route at the row's left keeps
/// [`ROUTE_RESERVE`] of them beside the full rung and [`ROUTE_RESERVE_MIN`] beside the shorter
/// one; the widest rung that fits beside its share is the one drawn, and when not even the
/// narrowest fits it is the keys alone.
pub fn hint(view: &View<'_>, room: usize) -> Hint {
    let [help, leave, accept] = caps(view);
    let full = present([help, leave, accept]);
    // Without a help cap (the help is open) the shorter rung is the same as the full one.
    let short = if help.is_some() {
        present([help, accept])
    } else {
        full.clone()
    };
    let caps = [(full, ROUTE_RESERVE), (short.clone(), ROUTE_RESERVE_MIN)]
        .into_iter()
        .find(|(rung, reserve)| spread(rung) + reserve <= room)
        .map(|(rung, _)| rung)
        .unwrap_or_else(|| bare(&short));
    Hint { caps }
}

/// The columns `caps` take, with the gaps between them.
pub fn spread(caps: &[Cap]) -> usize {
    let gaps = caps.len().saturating_sub(1) * HINT_GAP;
    caps.iter().map(|cap| cap.width()).sum::<usize>() + gaps
}

/// What the prompt strip says at its right: every host while the filter is empty, the matches of
/// the whole while it is not.
pub fn counter(view: &View<'_>) -> String {
    if view.query.is_empty() {
        match view.total {
            1 => "1 host".to_owned(),
            total => format!("{total} hosts"),
        }
    } else {
        format!("{} / {}", view.matched, view.total)
    }
}

/// Left to right: help, leave and accept, so the key that drops first is at the left.
fn caps(view: &View<'_>) -> [Option<Cap>; 3] {
    [
        (!view.help).then_some(Cap {
            label: "Keys",
            key: if view.query.is_empty() { "?" } else { "F1" },
        }),
        Some(Cap {
            label: if view.help { "Close" } else { "Quit" },
            key: "esc",
        }),
        (view.matched > 0).then_some(Cap {
            label: "Connect",
            key: "\u{21b5}",
        }),
    ]
}

fn present<const N: usize>(caps: [Option<Cap>; N]) -> Vec<Cap> {
    caps.into_iter().flatten().collect()
}

fn bare(rung: &[Cap]) -> Vec<Cap> {
    rung.iter()
        .map(|cap| Cap {
            label: "",
            key: cap.key,
        })
        .collect()
}

fn width(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use test_case::test_case;

    use super::*;
    use crate::keymap::BINDINGS;

    fn view(matched: usize, query: &str, help: bool) -> View<'_> {
        View {
            query,
            matched,
            total: matched.max(5),
            rows: Vec::new(),
            help,
            recents: false,
        }
    }

    fn spell(caps: &[Cap]) -> String {
        caps.iter()
            .map(|cap| {
                if cap.label.is_empty() {
                    cap.key.to_owned()
                } else {
                    format!("{} {}", cap.label, cap.key)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[test_case(5, "", false, 78, "Keys ?, Quit esc, Connect \u{21b5}"; "the whole ladder")]
    #[test_case(2, "consul", false, 78, "Keys F1, Quit esc, Connect \u{21b5}"; "? is a character once there is a query")]
    #[test_case(1, "lab-a", false, 78, "Keys F1, Quit esc, Connect \u{21b5}"; "one match is the same ladder")]
    #[test_case(0, "zzzz", false, 78, "Keys F1, Quit esc"; "no connect when nothing matches")]
    #[test_case(5, "", true, 78, "Close esc, Connect \u{21b5}"; "esc closes the help first, and the help key is not offered")]
    #[test_case(0, "zzzz", true, 78, "Close esc"; "esc closes the help with nothing matched")]
    #[test_case(5, "", false, 77, "Keys ?, Quit esc, Connect \u{21b5}"; "every key beside the route's room")]
    #[test_case(5, "", false, 76, "Keys ?, Connect \u{21b5}"; "Quit goes first")]
    #[test_case(5, "", false, 65, "Keys ?, Connect \u{21b5}"; "the second rung to the column")]
    #[test_case(5, "", false, 52, "Keys ?, Connect \u{21b5}"; "the labels keep their smaller reserve")]
    #[test_case(5, "", false, 44, "Keys ?, Connect \u{21b5}"; "the labels to the column")]
    #[test_case(5, "", false, 43, "?, \u{21b5}"; "the keys alone")]
    #[test_case(0, "zzzz", false, 0, "F1"; "the help key alone")]
    #[test_case(5, "", true, 9, "esc, \u{21b5}"; "the help open: the bare keys of the only rung")]
    fn the_ladder_drops_quit_first(
        matched: usize,
        query: &str,
        help: bool,
        room: usize,
        caps: &str,
    ) {
        let got = hint(&view(matched, query, help), room);
        assert_eq!(spell(&got.caps), caps);
    }

    fn every_state(mut check: impl FnMut(&View<'_>, usize, &Hint)) {
        for matched in 0..=3 {
            for query in ["", "x"] {
                for help in [false, true] {
                    for room in 0..=80 {
                        let view = view(matched, query, help);
                        check(&view, room, &hint(&view, room));
                    }
                }
            }
        }
    }

    #[test]
    fn at_any_width_the_row_fits_or_is_the_keys_alone() {
        every_state(|view, room, hint| {
            let used = spread(&hint.caps);
            let alone = hint.caps.iter().all(|cap| cap.label.is_empty());
            let reserve = if hint.caps.len() == 3 {
                ROUTE_RESERVE
            } else {
                ROUTE_RESERVE_MIN
            };
            assert!(
                used + reserve <= room || alone,
                "{} {:?} {} {room}: {hint:?}",
                view.matched,
                view.query,
                view.help
            );
            assert_eq!(
                hint.caps.iter().any(|cap| cap.key == "\u{21b5}"),
                view.matched > 0,
                "Connect stays to the last rung: {hint:?}"
            );
            assert_eq!(
                hint.caps
                    .iter()
                    .any(|cap| cap.key == "F1" || cap.key == "?"),
                !view.help,
                "the help key stays to the last rung while the help is closed: {hint:?}"
            );
            assert_eq!(
                hint.caps.iter().any(|cap| cap.key == "F1"),
                !view.help && !view.query.is_empty()
            );
            assert_eq!(
                hint.caps.iter().any(|cap| cap.key == "?"),
                !view.help && view.query.is_empty()
            );
        });
    }

    #[test]
    fn the_keys_are_ordered_so_the_ones_that_drop_first_are_at_the_left() {
        let view = view(5, "", false);
        let widest = hint(&view, 80);
        let order: Vec<_> = widest.caps.iter().map(|cap| cap.label).collect();
        assert_eq!(order, ["Keys", "Quit", "Connect"]);
        for room in 0..=80 {
            let caps = hint(&view, room).caps;
            let kept: Vec<_> = widest
                .caps
                .iter()
                .filter(|cap| caps.iter().any(|c| c.key == cap.key))
                .collect();
            assert_eq!(
                kept.len(),
                caps.len(),
                "{room}: only keys of the widest row"
            );
            assert!(
                kept.iter().all(|cap| cap.label != "Quit") || caps.len() == 3,
                "{room}: Quit drops before Keys and Connect"
            );
        }
    }

    #[test]
    fn every_key_a_rung_names_is_a_row_of_the_key_table() {
        let rows: BTreeSet<String> = BINDINGS
            .iter()
            .flat_map(|binding| binding.keys.split(", "))
            .map(str::to_lowercase)
            .collect();
        let mut named = BTreeSet::new();
        every_state(|_, _, hint| {
            for cap in &hint.caps {
                let names = match cap.key {
                    "\u{21b5}" => vec!["enter".to_owned()],
                    other => vec![other.to_lowercase()],
                };
                for name in names {
                    assert!(rows.contains(&name), "{name:?} is not in BINDINGS");
                    named.insert(name);
                }
            }
        });
        assert_eq!(
            named,
            ["?", "enter", "esc", "f1"].map(str::to_owned).into(),
            "every key the ladder can name was reached"
        );
    }

    //          matched total query => counter
    #[test_case(5, 5, "" => "5 hosts"; "every host")]
    #[test_case(1, 1, "" => "1 host"; "one host")]
    #[test_case(0, 0, "" => "0 hosts"; "no hosts")]
    #[test_case(2, 5, "con" => "2 / 5"; "matches of the whole")]
    #[test_case(0, 5, "zz" => "0 / 5"; "no match")]
    fn the_counter_counts(matched: usize, total: usize, query: &str) -> String {
        let view = View {
            query,
            matched,
            total,
            rows: Vec::new(),
            help: false,
            recents: false,
        };
        counter(&view)
    }
}
