use std::mem;
use std::ops::Range;

use crate::config::{Entry, Host};
use crate::rank::{Match, Ranker};

/// Query, matches, cursor and scroll for one session. The only owner of all four: the UI
/// keeps no selection or table offset of its own and builds each frame from [`Picker::view`].
pub struct Picker {
    ranker: Ranker,
    query: String,
    matches: Matches,
    scroll: usize,
}

/// A key with the terminal encoding stripped. `sshpick-ui` maps crossterm events to this one to
/// one, with no policy; what each key does lives in [`Picker::handle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A printable character, typed or pasted (Shift folded in).
    Char(char),
    /// Control plus a letter, lowercase: `Ctrl('c')`. In raw mode Ctrl-J (LF) and Ctrl-H arrive
    /// here, not as Enter or Backspace.
    Ctrl(char),
    Enter,
    Esc,
    Backspace,
    Up,
    Down,
}

/// How a session ends. The binary maps each to an exit path; see its `Exit` codes.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Enter on a match: restore the terminal, then exec `ssh -- <host>`.
    Pick(Host),
    /// Esc, Ctrl-C, Ctrl-G, Ctrl-Q, or Ctrl-D on an empty query: exit 130, as fzf.
    Cancel,
    /// Enter with nothing matched: exit 1, as fzf.
    NoMatch,
}

/// Everything one frame shows from the picker.
#[derive(Debug)]
pub struct View<'a> {
    pub query: &'a str,
    pub matched: usize,
    pub total: usize,
    /// The rows that fit, top to bottom as drawn: the best visible match first (fzf's
    /// `--layout=reverse`). Exactly one is `selected` unless nothing matched.
    pub rows: Vec<Row<'a>>,
    /// Matches past the last visible row, the "N more below" count.
    pub below: usize,
}

/// One visible row. Each column is its text cut into `(text, matched)` runs, in order, so the
/// UI can style matched characters without knowing how the haystack was laid out.
#[derive(Debug)]
pub struct Row<'a> {
    pub env: Vec<(&'a str, bool)>,
    pub host: Vec<(&'a str, bool)>,
    pub what: Vec<(&'a str, bool)>,
    pub selected: bool,
}

#[derive(Debug)]
enum Matches {
    None,
    Some {
        better: Vec<Match>,
        current: Match,
        worse: Vec<Match>,
    },
}

impl Picker {
    /// Rank `entries` against `query` (argv joined by spaces), cursor on the best match.
    pub fn new(entries: Vec<Entry>, query: &str) -> Picker {
        let mut ranker = Ranker::new(entries);
        let matches = Matches::best_first(ranker.rank(query));
        Picker {
            ranker,
            query: query.to_owned(),
            matches,
            scroll: 0,
        }
    }

    /// fzf `--select-1`: the host when exactly one entry matches, so the caller can exec before
    /// touching the terminal. Applies to any query, the empty one included (a one-host config).
    /// Zero matches is `None`: the UI opens, as fzf does.
    pub fn only_match(&self) -> Option<&Host> {
        match &self.matches {
            Matches::Some {
                better,
                current,
                worse,
            } if better.is_empty() && worse.is_empty() => Some(&self.ranker.entry(current).host),
            Matches::None | Matches::Some { .. } => None,
        }
    }

    /// The host under the cursor, `None` when nothing matches. The preview pane follows it.
    pub fn selected(&self) -> Option<&Host> {
        match &self.matches {
            Matches::Some { current, .. } => Some(&self.ranker.entry(current).host),
            Matches::None => None,
        }
    }

    /// Every match, best first, in the order [`Picker::view`] draws them top to bottom. The cursor
    /// and scroll do not move it; an empty query keeps file order. This is what `--list` prints.
    pub fn listing(&self) -> Vec<&Entry> {
        self.matches
            .ranked()
            .map(|(found, _)| self.ranker.entry(found))
            .collect()
    }

    /// Apply one key. `Some` ends the session. The bindings are fzf's defaults:
    ///
    /// | Key                                   | Effect                                     |
    /// |---------------------------------------|--------------------------------------------|
    /// | Enter                                 | `Pick(selected)`, or `NoMatch`             |
    /// | Esc, Ctrl-C, Ctrl-G, Ctrl-Q           | `Cancel`                                   |
    /// | Ctrl-D                                | `Cancel` on an empty query, else nothing   |
    /// | Up, Ctrl-P, Ctrl-K                    | cursor to the next better match (up the list) |
    /// | Down, Ctrl-N, Ctrl-J                  | cursor to the next worse match (down the list) |
    /// | printable                             | append to the query                        |
    /// | Backspace, Ctrl-H                     | delete the last char                       |
    /// | Ctrl-U                                | clear the query                            |
    /// | Ctrl-W                                | delete the last word                       |
    ///
    /// The query cursor is always at the end; there is no Left/Right editing.
    pub fn handle(&mut self, key: Key) -> Option<Outcome> {
        match key {
            Key::Enter => {
                return Some(match self.selected() {
                    Some(host) => Outcome::Pick(host.clone()),
                    None => Outcome::NoMatch,
                });
            }
            Key::Esc | Key::Ctrl('c' | 'g' | 'q') => return Some(Outcome::Cancel),
            Key::Ctrl('d') if self.query.is_empty() => return Some(Outcome::Cancel),
            Key::Up | Key::Ctrl('p' | 'k') => self.matches.toward_better(),
            Key::Down | Key::Ctrl('n' | 'j') => self.matches.toward_worse(),
            Key::Char(c) => self.edit(|query| query.push(c)),
            Key::Backspace | Key::Ctrl('h') => self.edit(|query| {
                query.pop();
            }),
            Key::Ctrl('u') => self.edit(String::clear),
            Key::Ctrl('w') => self.edit(delete_word),
            Key::Ctrl(_) => {}
        }
        None
    }

    /// The rows that fit in `height` lines, scrolled so the cursor is visible, plus the prompt
    /// and count. Takes `&mut self` because it moves the scroll window.
    pub fn view(&mut self, height: usize) -> View<'_> {
        self.scroll = follow(
            self.scroll,
            self.matches.cursor(),
            height,
            self.matches.len(),
        );
        let rows: Vec<_> = self
            .matches
            .ranked()
            .skip(self.scroll)
            .take(height)
            .map(|(found, selected)| {
                let entry = self.ranker.entry(found);
                Row {
                    env: runs(&entry.env, &found.hits.env),
                    host: runs(entry.host.as_str(), &found.hits.host),
                    what: runs(&entry.what, &found.hits.what),
                    selected,
                }
            })
            .collect();
        View {
            query: &self.query,
            matched: self.matches.len(),
            total: self.ranker.len(),
            below: self.matches.len().saturating_sub(self.scroll + rows.len()),
            rows,
        }
    }

    fn edit(&mut self, edit: impl FnOnce(&mut String)) {
        let before = self.query.clone();
        edit(&mut self.query);
        if self.query != before {
            self.matches = Matches::best_first(self.ranker.rank(&self.query));
            self.scroll = 0;
        }
    }
}

impl Matches {
    fn best_first(ranked: Vec<Match>) -> Matches {
        let mut worse = ranked;
        worse.reverse();
        match worse.pop() {
            Some(current) => Matches::Some {
                better: Vec::new(),
                current,
                worse,
            },
            None => Matches::None,
        }
    }

    fn toward_worse(&mut self) {
        if let Matches::Some {
            better,
            current,
            worse,
        } = self
            && let Some(next) = worse.pop()
        {
            better.push(mem::replace(current, next));
        }
    }

    fn toward_better(&mut self) {
        if let Matches::Some {
            better,
            current,
            worse,
        } = self
            && let Some(previous) = better.pop()
        {
            worse.push(mem::replace(current, previous));
        }
    }

    fn len(&self) -> usize {
        match self {
            Matches::Some { better, worse, .. } => better.len() + 1 + worse.len(),
            Matches::None => 0,
        }
    }

    fn cursor(&self) -> usize {
        match self {
            Matches::Some { better, .. } => better.len(),
            Matches::None => 0,
        }
    }

    fn ranked(&self) -> impl Iterator<Item = (&Match, bool)> {
        let (better, current, worse) = match self {
            Matches::Some {
                better,
                current,
                worse,
            } => (better.as_slice(), Some(current), worse.as_slice()),
            Matches::None => (&[][..], None, &[][..]),
        };
        let better = better.iter().map(|m| (m, false));
        let current = current.into_iter().map(|m| (m, true));
        let worse = worse.iter().rev().map(|m| (m, false));
        better.chain(current).chain(worse)
    }
}

fn follow(scroll: usize, cursor: usize, height: usize, len: usize) -> usize {
    if height == 0 {
        return scroll;
    }
    let scroll = scroll.min(cursor);
    let scroll = if cursor >= scroll + height {
        cursor + 1 - height
    } else {
        scroll
    };
    scroll.min(len.saturating_sub(height))
}

fn runs<'a>(text: &'a str, hits: &[Range<usize>]) -> Vec<(&'a str, bool)> {
    let mut runs = Vec::new();
    let mut at = 0;
    for hit in hits {
        if hit.start > at {
            runs.push((&text[at..hit.start], false));
        }
        runs.push((&text[hit.clone()], true));
        at = hit.end;
    }
    if at < text.len() {
        runs.push((&text[at..], false));
    }
    runs
}

fn delete_word(query: &mut String) {
    let end = query.trim_end().len();
    let start = query[..end]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(index, c)| index + c.len_utf8());
    query.truncate(start);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(hosts: &[&str]) -> Vec<Entry> {
        hosts
            .iter()
            .map(|host| Entry {
                env: "E".to_owned(),
                host: Host::literal(host).unwrap(),
                what: String::new(),
                aliases: Vec::new(),
            })
            .collect()
    }

    fn picker(hosts: &[&str]) -> Picker {
        Picker::new(entries(hosts), "")
    }

    fn type_text(picker: &mut Picker, text: &str) {
        for c in text.chars() {
            assert_eq!(picker.handle(Key::Char(c)), None);
        }
    }

    fn selected(picker: &Picker) -> &str {
        picker.selected().map_or("", Host::as_str)
    }

    fn drawn(picker: &mut Picker, height: usize) -> (Vec<String>, Option<usize>) {
        let view = picker.view(height);
        let hosts = view
            .rows
            .iter()
            .map(|r| r.host.iter().map(|(text, _)| *text).collect())
            .collect();
        (hosts, view.rows.iter().position(|r| r.selected))
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn cancel_keys_end_with_cancel() {
        for key in [Key::Esc, Key::Ctrl('c'), Key::Ctrl('g'), Key::Ctrl('q')] {
            let mut picker = picker(&["a"]);
            type_text(&mut picker, "a");
            assert_eq!(picker.handle(key), Some(Outcome::Cancel), "{key:?}");
        }
        let mut picker = picker(&["a"]);
        assert_eq!(picker.handle(Key::Ctrl('d')), Some(Outcome::Cancel));
        type_text(&mut picker, "a");
        assert_eq!(picker.handle(Key::Ctrl('d')), None);
        assert_eq!(picker.handle(Key::Ctrl('z')), None);
    }

    #[test]
    fn enter_picks_the_selection_and_no_match_is_no_match() {
        let mut picker = picker(&["a", "b"]);
        assert_eq!(
            picker.handle(Key::Enter),
            Some(Outcome::Pick(Host::literal("a").unwrap()))
        );
        type_text(&mut picker, "zzzz");
        assert_eq!(picker.selected(), None);
        assert_eq!(picker.handle(Key::Enter), Some(Outcome::NoMatch));
    }

    #[test]
    fn the_selection_starts_on_the_best_row_at_the_top() {
        let mut picker = picker(&["xxb", "a", "b"]);
        type_text(&mut picker, "b");
        assert_eq!(drawn(&mut picker, 5), (strings(&["b", "xxb"]), Some(0)));
    }

    #[test]
    fn down_moves_to_a_worse_match_and_up_to_a_better_one() {
        let mut picker = picker(&["a", "b", "c"]);
        assert_eq!(drawn(&mut picker, 5), (strings(&["a", "b", "c"]), Some(0)));
        picker.handle(Key::Up);
        assert_eq!(selected(&picker), "a", "nothing is better than the best");
        for want in ["b", "c", "c"] {
            picker.handle(Key::Down);
            assert_eq!(selected(&picker), want);
        }
        assert_eq!(drawn(&mut picker, 5).1, Some(2));
        for want in ["b", "a", "a"] {
            picker.handle(Key::Up);
            assert_eq!(selected(&picker), want);
        }
        for key in [Key::Ctrl('n'), Key::Ctrl('j')] {
            picker.handle(key);
        }
        assert_eq!(selected(&picker), "c");
        for key in [Key::Ctrl('p'), Key::Ctrl('k')] {
            picker.handle(key);
        }
        assert_eq!(selected(&picker), "a");
    }

    #[test]
    fn query_change_resets_cursor_and_scroll() {
        let names: Vec<String> = (0..20).map(|n| format!("h{n:02}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut picker = picker(&names);
        for _ in 0..15 {
            picker.handle(Key::Down);
        }
        let (visible, at) = drawn(&mut picker, 5);
        assert_eq!(visible, strings(&["h11", "h12", "h13", "h14", "h15"]));
        assert_eq!(
            at,
            Some(4),
            "the cursor is the worst visible row, at the bottom"
        );

        type_text(&mut picker, "h1");
        let (visible, at) = drawn(&mut picker, 5);
        assert_eq!(visible.first().map(String::as_str), Some("h10"));
        assert_eq!(at, Some(0), "best match is selected, drawn first");

        type_text(&mut picker, "5");
        assert_eq!(drawn(&mut picker, 5), (strings(&["h15"]), Some(0)));
    }

    #[test]
    fn an_edit_that_changes_nothing_keeps_the_cursor() {
        let mut picker = picker(&["a", "b", "c"]);
        picker.handle(Key::Down);
        for key in [
            Key::Backspace,
            Key::Ctrl('u'),
            Key::Ctrl('w'),
            Key::Ctrl('h'),
        ] {
            picker.handle(key);
        }
        assert_eq!(selected(&picker), "b");
    }

    #[test]
    fn only_match_needs_exactly_one_match() {
        let only = |hosts: &[&str], query: &str| {
            Picker::new(entries(hosts), query)
                .only_match()
                .map(|host| host.to_string())
        };
        assert_eq!(only(&["a"], ""), Some("a".to_owned()));
        assert_eq!(only(&["a", "b"], ""), None);
        assert_eq!(only(&["a", "b"], "b"), Some("b".to_owned()));
        assert_eq!(only(&["a", "b"], "zz"), None);
        assert_eq!(only(&[], ""), None);
    }

    #[test]
    fn listing_is_best_first_and_ignores_the_cursor() {
        let hosts = |picker: &Picker| -> Vec<String> {
            picker
                .listing()
                .iter()
                .map(|entry| entry.host.to_string())
                .collect()
        };
        let mut picker = picker(&["xxb", "a", "b"]);
        assert_eq!(hosts(&picker), ["xxb", "a", "b"], "file order when empty");

        picker.handle(Key::Down);
        picker.handle(Key::Down);
        assert_eq!(
            hosts(&picker),
            ["xxb", "a", "b"],
            "the cursor reorders nothing"
        );

        type_text(&mut picker, "b");
        assert_eq!(hosts(&picker), ["b", "xxb"], "ranked, not in file order");

        type_text(&mut picker, "zz");
        assert!(picker.listing().is_empty());
    }

    #[test]
    fn listing_is_the_view_order_read_top_down() {
        let names: Vec<String> = (0..12).map(|n| format!("h{n:02}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        for query in ["", "h1", "'h0", "!h0"] {
            let mut picker = Picker::new(entries(&names), query);
            for _ in 0..3 {
                picker.handle(Key::Down);
            }
            let listed: Vec<String> = picker
                .listing()
                .iter()
                .map(|entry| entry.host.to_string())
                .collect();
            let (drawn, _) = drawn(&mut picker, 20);
            assert!(!listed.is_empty(), "{query:?}");
            assert_eq!(drawn, listed, "{query:?}");
        }
    }

    #[test]
    fn editing_keys() {
        let mut picker = picker(&["a"]);
        type_text(&mut picker, "ab cd");
        assert_eq!(picker.view(1).query, "ab cd");
        picker.handle(Key::Backspace);
        assert_eq!(picker.view(1).query, "ab c");
        picker.handle(Key::Ctrl('w'));
        assert_eq!(picker.view(1).query, "ab ");
        picker.handle(Key::Ctrl('w'));
        assert_eq!(picker.view(1).query, "");
        type_text(&mut picker, "xy");
        picker.handle(Key::Ctrl('u'));
        assert_eq!(picker.view(1).query, "");
    }

    #[test]
    fn delete_word_is_unix_word_rubout() {
        let cases = [
            ("", ""),
            ("one", ""),
            ("one two", "one "),
            ("one two  ", "one "),
            ("  one", "  "),
            ("a b\u{2014}c", "a "),
        ];
        for (before, want) in cases {
            let mut query = before.to_owned();
            delete_word(&mut query);
            assert_eq!(query, want, "{before:?}");
        }
    }

    #[test]
    fn view_counts_and_cuts_hits_into_runs() {
        let entry = Entry {
            env: "LAB".to_owned(),
            host: Host::literal("lab-a").unwrap(),
            what: "build box".to_owned(),
            aliases: Vec::new(),
        };
        let mut picker = Picker::new(vec![entry], "^lab box");
        let view = picker.view(3);
        assert_eq!((view.matched, view.total, view.query), (1, 1, "^lab box"));
        let row = &view.rows[0];
        assert_eq!(row.env, [("LAB", true)]);
        assert_eq!(row.host, [("lab-a", false)]);
        assert_eq!(row.what, [("build ", false), ("box", true)]);
        assert!(row.selected);
    }

    #[test]
    fn view_reflows_when_the_window_grows() {
        let names: Vec<String> = (0..10).map(|n| format!("h{n}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut picker = picker(&names);
        for _ in 0..9 {
            picker.handle(Key::Down);
        }
        assert_eq!(
            drawn(&mut picker, 3),
            (strings(&["h7", "h8", "h9"]), Some(2))
        );
        let (visible, at) = drawn(&mut picker, 10);
        assert_eq!(visible.len(), 10);
        assert_eq!(at, Some(9));
    }

    #[test]
    fn view_counts_the_matches_below_the_window() {
        let mut picker = picker(&["a", "b", "c", "d", "e"]);
        assert_eq!(picker.view(5).below, 0, "all fit");
        assert_eq!(picker.view(3).below, 2);
        for _ in 0..3 {
            picker.handle(Key::Down);
        }
        assert_eq!(picker.view(3).below, 1, "the window followed the cursor");
        picker.handle(Key::Down);
        let view = picker.view(3);
        assert_eq!((view.below, view.matched), (0, 5), "scrolled to the end");
        assert_eq!(picker.view(1).below, 0, "the cursor row is the last one");
        picker.handle(Key::Up);
        assert_eq!(picker.view(1).below, 1);
        type_text(&mut picker, "zzzz");
        assert_eq!(picker.view(3).below, 0, "nothing matched");
    }

    #[test]
    fn follow_scrolls_like_fzf() {
        let cases = [
            ((0, 0, 5, 20), 0),
            ((0, 5, 5, 20), 1),
            ((3, 2, 5, 20), 2),
            ((10, 11, 5, 12), 7),
            ((4, 4, 0, 20), 4),
        ];
        for ((scroll, cursor, height, len), want) in cases {
            assert_eq!(follow(scroll, cursor, height, len), want);
        }
    }

    #[test]
    fn runs_round_trip_with_non_ascii() {
        let got = runs("LAB — x", &[0..1, 4..7]);
        assert_eq!(
            got,
            [("L", true), ("AB ", false), ("—", true), (" x", false)]
        );
        assert_eq!(runs("", &[]), []);
        assert_eq!(runs("abc", &[]), [("abc", false)]);
        let all = 0..3;
        assert_eq!(runs("abc", std::slice::from_ref(&all)), [("abc", true)]);
    }
}
