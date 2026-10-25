//! The hint row never loses accept and cancel: the window range is dropped before any key is.
//! `?` is offered only while the filter is empty; it is a filter character otherwise.

use crate::picker::View;

const GAP: &str = "   ";
const RANGE_DASH: char = '\u{2013}';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    pub keys: String,
    /// The window range, `6–17`, or empty when it does not fit beside the keys.
    pub status: String,
}

#[derive(Clone, Copy)]
struct Chip {
    key: &'static str,
    does: &'static str,
}

pub fn hint(view: &View<'_>, room: usize) -> Hint {
    let [movement, accept, leave, help] = chips(view);
    let narrowest = present([accept, leave]);
    let rungs = [
        present([movement, accept, leave, help]),
        present([movement, accept, leave]),
        narrowest.clone(),
    ];
    let keys = rungs
        .iter()
        .map(|rung| spell(rung))
        .find(|text| width(text) <= room)
        .unwrap_or_else(|| bare(&narrowest));
    let left = room.saturating_sub(width(&keys) + width(GAP));
    let status = view
        .range()
        .map(|(first, last)| format!("{first}{RANGE_DASH}{last}"))
        .filter(|text| width(text) <= left)
        .unwrap_or_default();
    Hint { keys, status }
}

fn chips(view: &View<'_>) -> [Option<Chip>; 4] {
    [
        (view.matched > 1).then_some(Chip {
            key: "\u{2191}\u{2193}",
            does: "move",
        }),
        (view.matched > 0).then_some(Chip {
            key: "enter",
            does: "ssh",
        }),
        Some(Chip {
            key: "esc",
            does: if view.help { "close" } else { "quit" },
        }),
        (!view.help).then_some(Chip {
            key: if view.query.is_empty() { "?" } else { "F1" },
            does: "keys",
        }),
    ]
}

fn present<const N: usize>(chips: [Option<Chip>; N]) -> Vec<Chip> {
    chips.into_iter().flatten().collect()
}

fn spell(rung: &[Chip]) -> String {
    rung.iter()
        .map(|chip| format!("{} {}", chip.key, chip.does))
        .collect::<Vec<_>>()
        .join(GAP)
}

fn bare(rung: &[Chip]) -> String {
    rung.iter()
        .map(|chip| chip.key)
        .collect::<Vec<_>>()
        .join(" ")
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
    use crate::picker::Row;

    fn view(matched: usize, query: &str, help: bool, above: usize, shown: usize) -> View<'_> {
        View {
            query,
            matched,
            total: matched.max(5),
            above,
            rows: (0..shown)
                .map(|_| Row {
                    env: Vec::new(),
                    host: Vec::new(),
                    what: Vec::new(),
                    selected: false,
                })
                .collect(),
            help,
        }
    }

    #[test_case(5, "", false, (0, 5), 78, "\u{2191}\u{2193} move   enter ssh   esc quit   ? keys", ""; "the whole ladder")]
    #[test_case(2, "consul", false, (0, 2), 78, "\u{2191}\u{2193} move   enter ssh   esc quit   F1 keys", ""; "? is a character once there is a query")]
    #[test_case(1, "lab-a", false, (0, 1), 78, "enter ssh   esc quit   F1 keys", ""; "nothing to move between")]
    #[test_case(0, "zzzz", false, (0, 0), 78, "esc quit   F1 keys", ""; "no enter when nothing matches")]
    #[test_case(5, "", true, (0, 5), 78, "\u{2191}\u{2193} move   enter ssh   esc close", ""; "esc closes the help first")]
    #[test_case(0, "zzzz", true, (0, 0), 78, "esc close", ""; "esc closes the help with nothing matched")]
    #[test_case(29, "", false, (0, 11), 78, "\u{2191}\u{2193} move   enter ssh   esc quit   ? keys", "1\u{2013}11"; "the window when the list scrolls")]
    #[test_case(29, "", false, (5, 11), 78, "\u{2191}\u{2193} move   enter ssh   esc quit   ? keys", "6\u{2013}16"; "the window follows the scroll")]
    #[test_case(5, "", false, (0, 5), 38, "\u{2191}\u{2193} move   enter ssh   esc quit", ""; "rung two when ? keys does not fit")]
    #[test_case(5, "", false, (0, 3), 38, "\u{2191}\u{2193} move   enter ssh   esc quit", "1\u{2013}3"; "the window needs room too")]
    #[test_case(29, "", false, (0, 11), 38, "\u{2191}\u{2193} move   enter ssh   esc quit", "1\u{2013}11"; "the window fits beside rung two")]
    #[test_case(29, "", false, (0, 11), 36, "\u{2191}\u{2193} move   enter ssh   esc quit", ""; "the window is dropped before a key is")]
    #[test_case(5, "", false, (0, 5), 22, "enter ssh   esc quit", ""; "rung three")]
    #[test_case(5, "", false, (0, 5), 8, "enter esc", ""; "bare key names")]
    #[test_case(0, "zzzz", false, (0, 0), 0, "esc", ""; "bare esc alone")]
    fn the_ladder_keeps_accept_and_cancel(
        matched: usize,
        query: &str,
        help: bool,
        (above, shown): (usize, usize),
        room: usize,
        keys: &str,
        status: &str,
    ) {
        let got = hint(&view(matched, query, help, above, shown), room);
        assert_eq!((got.keys.as_str(), got.status.as_str()), (keys, status));
    }

    fn every_state(mut check: impl FnMut(&View<'_>, usize, &Hint)) {
        for matched in 0..=3 {
            for query in ["", "x"] {
                for help in [false, true] {
                    for room in 0..=80 {
                        let view = view(matched, query, help, 0, matched.saturating_sub(1));
                        check(&view, room, &hint(&view, room));
                    }
                }
            }
        }
    }

    #[test]
    fn at_any_width_the_row_fits_or_is_the_bare_last_rung() {
        every_state(|view, room, hint| {
            let used = width(&hint.keys)
                + if hint.status.is_empty() {
                    0
                } else {
                    width(GAP) + width(&hint.status)
                };
            assert!(
                used <= room || width(&hint.keys) <= "enter esc".len(),
                "{} {:?} {} {room}: {hint:?}",
                view.matched,
                view.query,
                view.help
            );
            assert!(hint.keys.contains("esc"));
            assert_eq!(hint.keys.contains("enter"), view.matched > 0);
            assert_eq!(
                hint.keys.contains("F1"),
                hint.keys.contains("keys") && !view.query.is_empty()
            );
            assert_eq!(
                hint.keys.contains('?'),
                hint.keys.contains("keys") && view.query.is_empty()
            );
            if view.help {
                assert!(!hint.keys.contains("keys"));
            }
        });
    }

    #[test]
    fn every_key_a_rung_names_is_a_row_of_the_key_table() {
        const VERBS: [&str; 5] = ["move", "ssh", "quit", "close", "keys"];
        let rows: BTreeSet<String> = BINDINGS
            .iter()
            .flat_map(|binding| binding.keys.split(", "))
            .map(str::to_lowercase)
            .collect();
        let mut named = BTreeSet::new();
        every_state(|_, _, hint| {
            for word in hint.keys.split_whitespace().filter(|w| !VERBS.contains(w)) {
                let names = match word {
                    "\u{2191}\u{2193}" => vec!["up".to_owned(), "down".to_owned()],
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
            ["?", "down", "enter", "esc", "f1", "up"]
                .map(str::to_owned)
                .into(),
            "every key the ladder can name was reached"
        );
    }
}
