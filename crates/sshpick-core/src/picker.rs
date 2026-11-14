use std::mem;
use std::ops::Range;

use crate::config::{Entry, Host};
use crate::history::Recents;
use crate::rank::{Match, Ranker};

pub struct Picker {
    ranker: Ranker,
    query: String,
    matches: Matches,
    scroll: usize,
    help: bool,
}

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
    F(u8),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The whole entry, so a caller can print its ENV and WHAT as well as connect.
    Pick(Entry),
    Cancel,
    NoMatch,
}

#[derive(Debug)]
pub enum Resolution<'a> {
    /// Exactly one entry has a name or alias equal to the whole query, or the query is empty and
    /// the config has one host. Safe to act on without asking.
    One(&'a Entry),
    /// Exactly one entry matches, but only fuzzily: no name equals the query.
    Loose(&'a Entry),
    NoMatch,
    Many(usize),
}

#[derive(Debug)]
pub struct View<'a> {
    pub query: &'a str,
    pub matched: usize,
    pub total: usize,
    /// Matches scrolled off the top.
    pub above: usize,
    /// The rows that fit, top to bottom as drawn: the best visible match first (fzf's
    /// `--layout=reverse`). Exactly one is `selected` unless nothing matched.
    pub rows: Vec<Row<'a>>,
    pub help: bool,
    /// Some host, matched or not, has a last use, so LAST has something to show. Fixed for the
    /// session: the column never appears or disappears as the query changes.
    pub recents: bool,
}

impl View<'_> {
    /// The 1-based, inclusive window of matches on screen, only when it is not all of them.
    pub fn range(&self) -> Option<(usize, usize)> {
        (self.matched > self.rows.len() && !self.rows.is_empty())
            .then(|| (self.above + 1, self.above + self.rows.len()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run<'a> {
    pub text: &'a str,
    pub matched: bool,
}

#[derive(Debug)]
pub struct Row<'a> {
    pub env: Vec<Run<'a>>,
    pub host: Vec<Run<'a>>,
    /// `2h`, or `""` for a host never connected to. Not searchable, so never matched.
    pub last: &'a str,
    pub what: Vec<Run<'a>>,
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
    pub fn new(entries: Vec<Entry>, query: &str, recents: &Recents) -> Picker {
        let mut ranker = Ranker::new(entries, recents);
        let matches = Matches::best_first(ranker.rank(query));
        Picker {
            ranker,
            query: query.to_owned(),
            matches,
            scroll: 0,
            help: false,
        }
    }

    /// What the query settles before any terminal is touched. A query equal to exactly one host's
    /// name or alias is that host, whatever else matches (`ssh web` goes to `web`, not `web-2`).
    /// Otherwise fzf's `--select-1`: one match is `Loose`, except under the empty query, where a
    /// one-host config leaves nothing to guess.
    pub fn resolve(&self) -> Resolution<'_> {
        let listing = self.listing();
        let mut named = listing
            .iter()
            .copied()
            .filter(|entry| entry.names().any(|name| name == self.query));
        if let (Some(entry), None) = (named.next(), named.next()) {
            return Resolution::One(entry);
        }
        match listing[..] {
            [] => Resolution::NoMatch,
            [entry] if self.query.is_empty() => Resolution::One(entry),
            [entry] => Resolution::Loose(entry),
            _ => Resolution::Many(listing.len()),
        }
    }

    pub fn selected(&self) -> Option<&Host> {
        match &self.matches {
            Matches::Some { current, .. } => Some(&self.ranker.entry(current).host),
            Matches::None => None,
        }
    }

    /// Every match, best first, in the order [`Picker::view`] draws them top to bottom. The cursor
    /// and scroll do not move it; with `Recents::none()` an empty query keeps file order.
    pub fn listing(&self) -> Vec<&Entry> {
        self.matches
            .ranked()
            .map(|(found, _)| self.ranker.entry(found))
            .collect()
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn help_open(&self) -> bool {
        self.help
    }

    /// Apply one key. `Some` ends the session.
    pub fn handle(&mut self, key: Key) -> Option<Outcome> {
        match key {
            Key::Enter => {
                return Some(match &self.matches {
                    Matches::Some { current, .. } => {
                        Outcome::Pick(self.ranker.entry(current).clone())
                    }
                    Matches::None => Outcome::NoMatch,
                });
            }
            Key::Esc if self.help => self.help = false,
            Key::Esc | Key::Ctrl('c' | 'g' | 'q') => return Some(Outcome::Cancel),
            Key::Ctrl('d') if self.query.is_empty() => return Some(Outcome::Cancel),
            Key::Up | Key::Ctrl('p' | 'k') => self.matches.toward_better(),
            Key::Down | Key::Ctrl('n' | 'j') => self.matches.toward_worse(),
            Key::F(1) => self.help = !self.help,
            Key::Char('?') if self.query.is_empty() => self.help = !self.help,
            Key::Char(c) => self.edit(|query| query.push(c)),
            Key::Backspace | Key::Ctrl('h') => self.edit(|query| {
                query.pop();
            }),
            Key::Ctrl('u') => self.edit(String::clear),
            Key::Ctrl('w') => self.edit(delete_word),
            Key::Ctrl(_) | Key::F(_) => {}
        }
        None
    }

    pub fn follow(&mut self, height: usize) {
        self.scroll = scroll_to_show(
            self.scroll,
            self.matches.cursor(),
            height,
            self.matches.len(),
        );
    }

    pub fn view(&self, height: usize) -> View<'_> {
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
                    last: self.ranker.ago(found),
                    what: runs(&entry.what, &found.hits.what),
                    selected,
                }
            })
            .collect();
        View {
            query: &self.query,
            matched: self.matches.len(),
            total: self.ranker.len(),
            above: self.scroll,
            rows,
            help: self.help,
            recents: self.ranker.remembers(),
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
        {
            if let Some(next) = worse.pop() {
                better.push(mem::replace(current, next));
            } else if !better.is_empty() {
                let last = mem::replace(current, better.remove(0));
                worse.push(last);
                worse.extend(better.drain(..).rev());
            }
        }
    }

    fn toward_better(&mut self) {
        if let Matches::Some {
            better,
            current,
            worse,
        } = self
        {
            if let Some(previous) = better.pop() {
                worse.push(mem::replace(current, previous));
            } else if !worse.is_empty() {
                let first = mem::replace(current, worse.remove(0));
                better.push(first);
                better.extend(worse.drain(..).rev());
            }
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

fn scroll_to_show(scroll: usize, cursor: usize, height: usize, len: usize) -> usize {
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

fn runs<'a>(text: &'a str, hits: &[Range<usize>]) -> Vec<Run<'a>> {
    let mut runs = Vec::new();
    let mut at = 0;
    for hit in hits {
        if hit.start > at {
            runs.push(Run {
                text: &text[at..hit.start],
                matched: false,
            });
        }
        runs.push(Run {
            text: &text[hit.clone()],
            matched: true,
        });
        at = hit.end;
    }
    if at < text.len() {
        runs.push(Run {
            text: &text[at..],
            matched: false,
        });
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
    use std::time::{Duration, UNIX_EPOCH};

    use test_case::test_case;

    use super::*;

    fn run(text: &str, matched: bool) -> Run<'_> {
        Run { text, matched }
    }

    /// One entry per `"host alias... # comment"`; the aliases and the comment are optional.
    fn entries(hosts: &[&str]) -> Vec<Entry> {
        hosts
            .iter()
            .map(|line| {
                let (names, what) = line.split_once(" # ").unwrap_or((line, ""));
                let mut names = names.split(' ');
                Entry {
                    env: "E".to_owned(),
                    host: Host::literal(names.next().unwrap()).unwrap(),
                    what: what.to_owned(),
                    aliases: names.map(str::to_owned).collect(),
                    hostname: None,
                    user: None,
                    hop: None,
                }
            })
            .collect()
    }

    fn picker(hosts: &[&str]) -> Picker {
        Picker::new(entries(hosts), "", &Recents::none())
    }

    fn type_text(picker: &mut Picker, text: &str) {
        for c in text.chars() {
            assert_eq!(picker.handle(Key::Char(c)), None);
        }
    }

    fn range(picker: &mut Picker, height: usize) -> Option<(usize, usize)> {
        picker.follow(height);
        picker.view(height).range()
    }

    fn picked(outcome: Option<Outcome>) -> Option<String> {
        match outcome {
            Some(Outcome::Pick(entry)) => Some(entry.host.to_string()),
            _ => None,
        }
    }

    fn selected(picker: &Picker) -> &str {
        picker.selected().map_or("", Host::as_str)
    }

    fn drawn(picker: &mut Picker, height: usize) -> (Vec<String>, Option<usize>) {
        picker.follow(height);
        let view = picker.view(height);
        let hosts = view
            .rows
            .iter()
            .map(|r| r.host.iter().map(|run| run.text).collect())
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
        assert_eq!(picked(picker.handle(Key::Enter)).as_deref(), Some("a"));
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
        for want in ["b", "c"] {
            picker.handle(Key::Down);
            assert_eq!(selected(&picker), want);
        }
        assert_eq!(drawn(&mut picker, 5).1, Some(2));
        picker.handle(Key::Up);
        assert_eq!(selected(&picker), "b");
        for key in [
            Key::Ctrl('n'),
            Key::Ctrl('p'),
            Key::Ctrl('j'),
            Key::Ctrl('k'),
        ] {
            picker.handle(key);
        }
        assert_eq!(selected(&picker), "b");
    }

    #[test]
    fn moving_past_either_end_wraps_and_keeps_the_order() {
        let mut picker = picker(&["a", "b", "c", "d"]);
        picker.handle(Key::Up);
        assert_eq!(
            selected(&picker),
            "d",
            "up from the best wraps to the worst"
        );
        assert_eq!(
            drawn(&mut picker, 2),
            (strings(&["c", "d"]), Some(1)),
            "the window follows the wrap to the bottom"
        );
        assert_eq!(picker.view(2).range(), Some((3, 4)));
        picker.handle(Key::Down);
        assert_eq!(
            selected(&picker),
            "a",
            "down from the worst wraps to the best"
        );
        assert_eq!(drawn(&mut picker, 2), (strings(&["a", "b"]), Some(0)));
        for want in ["b", "c", "d", "a"] {
            picker.handle(Key::Down);
            assert_eq!(selected(&picker), want);
        }
        for want in ["d", "c", "b", "a", "d"] {
            picker.handle(Key::Up);
            assert_eq!(selected(&picker), want);
        }
        assert_eq!(drawn(&mut picker, 9).0, strings(&["a", "b", "c", "d"]));
    }

    #[test]
    fn one_match_and_no_match_stay_put() {
        let mut picker = picker(&["a"]);
        for key in [Key::Up, Key::Down] {
            picker.handle(key);
            assert_eq!(selected(&picker), "a");
        }
        type_text(&mut picker, "zzz");
        for key in [Key::Up, Key::Down] {
            picker.handle(key);
        }
        assert_eq!(drawn(&mut picker, 5), (Vec::new(), None));
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

    #[test_case(&["a"], "" => "one a"; "a one-host config")]
    #[test_case(&["a", "b"], "" => "many 2"; "empty query and two hosts")]
    #[test_case(&["a", "b"], "b" => "one b"; "exact name")]
    #[test_case(&["a", "b"], "zz" => "none"; "no match")]
    #[test_case(&[], "" => "none"; "no hosts")]
    #[test_case(&["web", "web-2", "db"], "web" => "one web"; "exact beats a prefix match")]
    #[test_case(&["web1", "web2", "db"], "web" => "many 2"; "two loose matches")]
    #[test_case(&["stg-consul1 consul1.stg", "other"], "consul1.stg" => "one stg-consul1"; "alias")]
    #[test_case(&["db-prod # Postgres primary", "web"], "postgres" => "loose db-prod"; "only through the comment")]
    #[test_case(&["web", "web"], "web" => "many 2"; "two entries share the name")]
    fn resolve_settles_a_query_without_the_picker(hosts: &[&str], query: &str) -> String {
        match Picker::new(entries(hosts), query, &Recents::none()).resolve() {
            Resolution::One(entry) => format!("one {}", entry.host),
            Resolution::Loose(entry) => format!("loose {}", entry.host),
            Resolution::NoMatch => "none".to_owned(),
            Resolution::Many(count) => format!("many {count}"),
        }
    }

    #[test]
    fn resolve_counts_matches_whatever_the_cursor_did() {
        let mut picker = picker(&["a", "b", "c"]);
        picker.handle(Key::Down);
        picker.handle(Key::Down);
        assert!(matches!(picker.resolve(), Resolution::Many(3)));
        type_text(&mut picker, "c");
        assert!(matches!(picker.resolve(), Resolution::One(entry) if entry.host.as_str() == "c"));
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

    fn recents(names: &[(&str, u64)]) -> Recents {
        let now = UNIX_EPOCH + Duration::from_secs(1_794_556_800);
        Recents::of(
            names
                .iter()
                .map(|&(name, ago)| (name, now - Duration::from_secs(ago))),
            now,
        )
    }

    #[test]
    fn the_cursor_starts_on_the_host_used_last_and_the_rows_say_when() {
        let used = recents(&[("c", 7200), ("b", 60)]);
        let mut picker = Picker::new(entries(&["a", "b", "c"]), "", &used);
        assert_eq!(selected(&picker), "b");
        picker.follow(10);
        let view = picker.view(10);
        assert!(view.recents);
        let rows: Vec<_> = view.rows.iter().map(|row| row.last).collect();
        assert_eq!(rows, ["1m", "2h", ""]);
        assert_eq!(drawn(&mut picker, 10).0, ["b", "c", "a"]);
    }

    #[test]
    fn the_view_has_no_age_when_no_host_has_one() {
        let mut picker = picker(&["a", "b"]);
        picker.follow(10);
        let view = picker.view(10);
        assert!(!view.recents);
        assert!(view.rows.iter().all(|row| row.last.is_empty()));
        let other = Picker::new(entries(&["a", "b"]), "", &recents(&[("gone", 5)]));
        assert!(!other.view(10).recents, "a name no host has is no history");
    }

    #[test]
    fn the_age_column_is_there_for_the_whole_session_whatever_the_query() {
        let used = recents(&[("b", 60)]);
        let mut picker = Picker::new(entries(&["a", "b"]), "", &used);
        type_text(&mut picker, "a");
        let view = picker.view(10);
        assert!(view.recents, "no matched row has an age, the column stays");
        assert!(view.rows.iter().all(|row| row.last.is_empty()));
    }

    #[test_case(""; "empty")]
    #[test_case("web"; "a prefix")]
    #[test_case("web-2"; "an exact name")]
    #[test_case("db"; "one loose match")]
    #[test_case("zz"; "nothing")]
    fn history_changes_the_order_and_never_what_resolve_says(query: &str) {
        let hosts = ["web", "web-2", "web-3", "db"];
        let used = recents(&[("web-3", 5), ("db", 9)]);
        let with = Picker::new(entries(&hosts), query, &used);
        let without = Picker::new(entries(&hosts), query, &Recents::none());
        let shape = |picker: &Picker| match picker.resolve() {
            Resolution::One(entry) => format!("one {}", entry.host),
            Resolution::Loose(entry) => format!("loose {}", entry.host),
            Resolution::NoMatch => "none".to_owned(),
            Resolution::Many(count) => format!("many {count}"),
        };
        assert_eq!(shape(&with), shape(&without));
        let sorted = |picker: &Picker| {
            let mut names: Vec<String> = picker
                .listing()
                .iter()
                .map(|e| e.host.to_string())
                .collect();
            names.sort();
            names
        };
        assert_eq!(sorted(&with), sorted(&without));
    }

    #[test]
    fn listing_is_the_view_order_read_top_down() {
        let names: Vec<String> = (0..12).map(|n| format!("h{n:02}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        for query in ["", "h1", "'h0", "!h0"] {
            let mut picker = Picker::new(entries(&names), query, &Recents::none());
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
    fn f1_toggles_the_help_and_question_mark_does_only_on_an_empty_query() {
        let mut picker = picker(&["a", "b"]);
        assert!(!picker.help_open());
        assert_eq!(picker.handle(Key::F(1)), None);
        assert!(picker.help_open());
        picker.handle(Key::F(1));
        assert!(!picker.help_open());

        picker.handle(Key::Char('?'));
        assert!(picker.help_open(), "? on an empty query opens it");
        picker.handle(Key::Char('?'));
        assert!(!picker.help_open(), "and closes it");
        assert_eq!(picker.view(1).query, "", "? is not typed on an empty query");

        type_text(&mut picker, "a");
        picker.handle(Key::Char('?'));
        assert!(!picker.help_open());
        assert_eq!(picker.view(1).query, "a?", "with a query, ? is a character");
        picker.handle(Key::F(1));
        assert!(picker.help_open(), "F1 works whatever the query");
    }

    #[test]
    fn other_function_keys_do_nothing() {
        let mut picker = picker(&["a", "b"]);
        for n in (2..=12).chain([0]) {
            assert_eq!(picker.handle(Key::F(n)), None);
        }
        assert!(!picker.help_open());
        assert_eq!(selected(&picker), "a");
    }

    #[test]
    fn esc_closes_the_help_before_it_cancels() {
        let mut picker = picker(&["a", "b"]);
        picker.handle(Key::F(1));
        assert_eq!(picker.handle(Key::Esc), None);
        assert!(!picker.help_open());
        assert_eq!(picker.handle(Key::Esc), Some(Outcome::Cancel));
    }

    #[test]
    fn the_cancel_chords_quit_even_with_the_help_open() {
        for key in [
            Key::Ctrl('c'),
            Key::Ctrl('g'),
            Key::Ctrl('q'),
            Key::Ctrl('d'),
        ] {
            let mut picker = picker(&["a"]);
            picker.handle(Key::F(1));
            assert_eq!(picker.handle(key), Some(Outcome::Cancel), "{key:?}");
        }
    }

    #[test]
    fn the_help_is_not_modal() {
        let mut picker = picker(&["a-1", "a-2", "b"]);
        picker.handle(Key::F(1));
        type_text(&mut picker, "a");
        assert!(picker.help_open(), "typing keeps it open");
        picker.handle(Key::Down);
        assert_eq!(selected(&picker), "a-2", "so do the arrows");
        assert_eq!(
            picked(picker.handle(Key::Enter)).as_deref(),
            Some("a-2"),
            "and Enter still picks"
        );
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
            host: Host::literal("web-a").unwrap(),
            what: "build box".to_owned(),
            aliases: Vec::new(),
            hostname: None,
            user: None,
            hop: None,
        };
        let picker = Picker::new(vec![entry], "^lab box", &Recents::none());
        let view = picker.view(3);
        assert_eq!((view.matched, view.total, view.query), (1, 1, "^lab box"));
        let row = &view.rows[0];
        assert_eq!(row.env, [run("LAB", true)]);
        assert_eq!(row.host, [run("web-a", false)]);
        assert_eq!(row.what, [run("build ", false), run("box", true)]);
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
    fn view_gives_the_window_of_matches_on_screen_only_when_it_is_not_all_of_them() {
        let mut picker = picker(&["a", "b", "c", "d", "e"]);
        assert_eq!(range(&mut picker, 5), None, "all fit");
        assert_eq!(range(&mut picker, 3), Some((1, 3)));
        for _ in 0..3 {
            picker.handle(Key::Down);
        }
        assert_eq!(
            range(&mut picker, 3),
            Some((2, 4)),
            "the window followed the cursor"
        );
        picker.handle(Key::Down);
        picker.follow(3);
        let view = picker.view(3);
        assert_eq!(
            (view.above, view.matched, view.range()),
            (2, 5, Some((3, 5))),
            "scrolled to the end"
        );
        assert_eq!(
            range(&mut picker, 1),
            Some((5, 5)),
            "the cursor row is the last one"
        );
        picker.handle(Key::Up);
        assert_eq!(range(&mut picker, 1), Some((4, 4)));
        assert_eq!(range(&mut picker, 0), None, "no room, no window");
        type_text(&mut picker, "zzzz");
        assert_eq!(range(&mut picker, 3), None, "nothing matched");
    }

    #[test]
    fn view_says_whether_the_help_is_open() {
        let mut picker = picker(&["a", "b"]);
        assert!(!picker.view(2).help);
        picker.handle(Key::F(1));
        assert!(picker.view(2).help);
        picker.handle(Key::Esc);
        assert!(!picker.view(2).help);
    }

    #[test]
    fn the_window_moves_on_follow_and_view_leaves_it_alone() {
        let mut picker = picker(&["a", "b", "c", "d", "e"]);
        for _ in 0..4 {
            picker.handle(Key::Down);
        }
        let before = picker.view(2);
        assert!(
            before.rows.iter().all(|row| !row.selected),
            "view alone does not scroll to the cursor"
        );
        assert_eq!((before.above, before.range()), (0, Some((1, 2))));
        assert_eq!(
            drawn(&mut picker, 2),
            (strings(&["d", "e"]), Some(1)),
            "follow brings the cursor into view"
        );
    }

    #[test]
    fn scrolling_keeps_the_cursor_in_view_like_fzf() {
        let cases = [
            ((0, 0, 5, 20), 0),
            ((0, 5, 5, 20), 1),
            ((3, 2, 5, 20), 2),
            ((10, 11, 5, 12), 7),
            ((4, 4, 0, 20), 4),
        ];
        for ((scroll, cursor, height, len), want) in cases {
            assert_eq!(scroll_to_show(scroll, cursor, height, len), want);
        }
    }

    #[test]
    fn runs_round_trip_with_non_ascii() {
        let got = runs("LAB — x", &[0..1, 4..7]);
        assert_eq!(
            got,
            [
                run("L", true),
                run("AB ", false),
                run("—", true),
                run(" x", false)
            ]
        );
        assert_eq!(runs("", &[]), []);
        assert_eq!(runs("abc", &[]), [run("abc", false)]);
        let all = 0..3;
        assert_eq!(runs("abc", std::slice::from_ref(&all)), [run("abc", true)]);
    }
}
