/// One row of the key table: what to press, and what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    /// The keys that do it, separated by `, `.
    pub keys: &'static str,
    pub does: &'static str,
}

/// Every key [`Picker::handle`](crate::Picker::handle) acts on. The popup, `sshpick pick --help`
/// and the README table all read this one list, and the tests below fail when a key does
/// something that has no row or a row names a key that does nothing.
pub const BINDINGS: &[Binding] = &[
    Binding {
        keys: "Enter",
        does: "ssh to the selected host",
    },
    Binding {
        keys: "Esc, Ctrl-C, Ctrl-G, Ctrl-Q",
        does: "quit (exit 130); Esc closes this help first",
    },
    Binding {
        keys: "Up, Ctrl-P, Ctrl-K",
        does: "select the previous host (up the list)",
    },
    Binding {
        keys: "Down, Ctrl-N, Ctrl-J",
        does: "select the next host (down the list)",
    },
    Binding {
        keys: "type",
        does: "filter; 'exact ^prefix suffix$ !not",
    },
    Binding {
        keys: "Backspace, Ctrl-H",
        does: "delete the last character",
    },
    Binding {
        keys: "Ctrl-U",
        does: "clear the filter",
    },
    Binding {
        keys: "Ctrl-W",
        does: "delete the last word",
    },
    Binding {
        keys: "Ctrl-D",
        does: "quit when the filter is empty",
    },
    Binding {
        keys: "?, F1",
        does: "toggle this help (? only on an empty filter)",
    },
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::config::{Entry, Host};
    use crate::picker::{Key, Picker};

    fn picker(query: &str) -> Picker {
        let entries = ["a-one", "a-two", "a-three"]
            .iter()
            .map(|host| Entry {
                env: String::new(),
                host: Host::literal(host).unwrap(),
                what: String::new(),
                aliases: Vec::new(),
            })
            .collect();
        let mut picker = Picker::new(entries, query);
        picker.handle(Key::Down);
        picker
    }

    /// How a key is written in the table.
    fn name(key: Key) -> String {
        match key {
            Key::Char('?') => "?".to_owned(),
            Key::Char(_) => "type".to_owned(),
            Key::Ctrl(c) => format!("Ctrl-{}", c.to_ascii_uppercase()),
            Key::F(n) => format!("F{n}"),
            Key::Enter => "Enter".to_owned(),
            Key::Esc => "Esc".to_owned(),
            Key::Backspace => "Backspace".to_owned(),
            Key::Up => "Up".to_owned(),
            Key::Down => "Down".to_owned(),
        }
    }

    /// Whether the key does anything to a picker with a query, with an empty one, or with its
    /// help open.
    fn acts(key: Key) -> bool {
        ["", "a", "zz"].into_iter().any(|query| {
            [false, true].into_iter().any(|help| {
                let mut picker = picker(query);
                if help {
                    picker.handle(Key::F(1));
                }
                let before = state(&mut picker);
                picker.handle(key).is_some() || state(&mut picker) != before
            })
        })
    }

    fn state(picker: &mut Picker) -> (String, Option<String>, bool) {
        (
            picker.view(10).query.to_owned(),
            picker.selected().map(ToString::to_string),
            picker.help_open(),
        )
    }

    fn every_key() -> Vec<Key> {
        let mut keys = vec![
            Key::Enter,
            Key::Esc,
            Key::Backspace,
            Key::Up,
            Key::Down,
            Key::Char('x'),
            Key::Char('?'),
        ];
        keys.extend(('a'..='z').map(Key::Ctrl));
        keys.extend((1..=12).map(Key::F));
        keys
    }

    fn rows() -> BTreeSet<&'static str> {
        BINDINGS
            .iter()
            .flat_map(|binding| binding.keys.split(", "))
            .collect()
    }

    #[test]
    fn every_key_the_picker_acts_on_has_a_row() {
        let rows = rows();
        for key in every_key() {
            if acts(key) {
                assert!(rows.contains(name(key).as_str()), "{key:?} has no row");
            }
        }
    }

    #[test]
    fn every_row_names_a_key_the_picker_acts_on() {
        let acted: BTreeSet<String> = every_key()
            .into_iter()
            .filter(|key| acts(*key))
            .map(name)
            .collect();
        for row in rows() {
            assert!(acted.contains(row), "{row} does nothing");
        }
    }

    #[test]
    fn rows_are_written_the_same_way_everywhere() {
        for binding in BINDINGS {
            assert!(!binding.keys.is_empty() && !binding.does.is_empty());
            assert_eq!(binding.keys, binding.keys.trim());
            assert!(!binding.does.ends_with('.'), "{}", binding.does);
        }
    }

    #[test]
    fn the_readme_lists_every_row() {
        let readme = include_str!("../../../README.md");
        for binding in BINDINGS {
            let listed = readme
                .lines()
                .any(|line| line.contains(binding.keys) && line.contains(binding.does));
            assert!(listed, "the README's key table lacks: {binding:?}");
        }
    }
}
