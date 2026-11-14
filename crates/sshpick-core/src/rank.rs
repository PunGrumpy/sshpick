use std::cmp::Reverse;
use std::ops::Range;

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::config::Entry;
use crate::history::{Last, Recents};

#[derive(Debug)]
pub(crate) struct Ranker {
    items: Vec<Item>,
    matcher: Matcher,
    chars: Vec<char>,
    indices: Vec<u32>,
}

#[derive(Debug)]
struct Item {
    entry: Entry,
    shown_len: usize,
    /// Joined once, when the ranker is built. Ranking and drawing never look a name up again.
    last: Option<Last>,
}

/// Where a word matched. `Alias` and `Written` are searched but never drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Env,
    Host,
    What,
    Alias,
    /// `HostName` or `User`.
    Written,
}

/// The fields one word looks in, chosen once per query for all hosts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    /// ENV, HOST, WHAT and aliases.
    Names,
    /// `HostName` and `User`, only for a word no name field of any host matches. Every address
    /// holds digits and dots, so `consul 1` would otherwise find every consul node whose address
    /// has a 1, and `stg` every host whose user is `postgres`.
    Written,
}

/// One word of the query, matched inside one field at a time.
#[derive(Debug)]
struct Word {
    /// Always positive: `negative` says what a match means.
    atom: Atom,
    negative: bool,
    rule: Rule,
    scope: Scope,
}

#[derive(Debug)]
enum Rule {
    /// nucleo's own match.
    Nucleo,
    /// A plain word that no field of any host contains: fuzzy, but each matched character starts
    /// a word or directly follows the previous one, so `pdb` finds `prod-db-1` and not
    /// `prod-bastion`.
    WordStarts,
    /// A `^`, `$` or `^…$` word with a capital. nucleo-matcher 0.3.1 compares the whole field
    /// instead of the anchored slice when the field and the word are ASCII and case matters, so
    /// such a word matches case-folded and the matched slice is compared here.
    Cased { folded: Atom, needle: Box<[u8]> },
}

#[derive(Debug)]
pub(crate) struct Match {
    item: usize,
    pub(crate) hits: Hits,
}

#[derive(Debug, Default)]
pub(crate) struct Hits {
    pub(crate) env: Vec<Range<usize>>,
    pub(crate) host: Vec<Range<usize>>,
    pub(crate) what: Vec<Range<usize>>,
}

impl Item {
    fn new(entry: Entry, recents: &Recents) -> Item {
        let last = recents.last(&entry);
        let shown = format!("{}\t{}\t{}", entry.env, entry.host, entry.what);
        Item {
            shown_len: shown.trim().chars().count(),
            entry,
            last,
        }
    }

    fn last_at(&self) -> Reverse<Option<u64>> {
        Reverse(self.last.as_ref().map(|last| last.at))
    }

    fn fields(&self, scope: Scope) -> impl Iterator<Item = (Field, &str)> {
        let entry = &self.entry;
        let aliases = entry
            .aliases
            .iter()
            .map(|alias| (Field::Alias, alias.as_str()));
        let written = (entry.hostname.iter().chain(&entry.user))
            .map(|value| (Field::Written, value.as_str()));
        [
            (Field::Env, entry.env.as_str()),
            (Field::Host, entry.host.as_str()),
            (Field::What, entry.what.as_str()),
        ]
        .into_iter()
        .chain(aliases)
        .chain(written)
        .filter(move |&(field, _)| (field == Field::Written) == (scope == Scope::Written))
    }
}

impl Word {
    /// Every word of the query, operators parsed. `user@host` is two words, as in an ssh
    /// command line from shell history.
    fn parse(query: &str) -> Vec<Word> {
        let mut words = Vec::new();
        for token in tokens(query) {
            match token.rsplit_once('@') {
                Some((user, host)) if !user.is_empty() && !host.is_empty() => {
                    words.extend([user, host].into_iter().filter_map(Word::new));
                }
                _ => words.extend(Word::new(token)),
            }
        }
        words
    }

    fn new(token: &str) -> Option<Word> {
        let mut atom = Atom::parse(token, CaseMatching::Smart, Normalization::Smart);
        if atom.needle_text().is_empty() {
            return None;
        }
        let negative = atom.negative;
        atom.negative = false;
        let rule = match (atom.kind, atom.needle_text()) {
            (AtomKind::Prefix | AtomKind::Postfix | AtomKind::Exact, Utf32Str::Ascii(needle))
                if needle.iter().any(u8::is_ascii_uppercase) =>
            {
                let text = String::from_utf8_lossy(needle);
                Rule::Cased {
                    folded: Atom::new(
                        &text,
                        CaseMatching::Ignore,
                        Normalization::Smart,
                        atom.kind,
                        false,
                    ),
                    needle: needle.into(),
                }
            }
            _ => Rule::Nucleo,
        };
        Some(Word {
            atom,
            negative,
            rule,
            scope: Scope::Names,
        })
    }

    /// A plain word is one with no operator: it matches as a substring, or by word starts.
    fn is_plain(&self) -> bool {
        self.atom.kind == AtomKind::Fuzzy && !self.negative
    }

    /// The same needle, matched as a substring.
    fn substring(&self) -> Word {
        let text: String = self.atom.needle_text().chars().collect();
        Word {
            atom: Atom::new(
                &text,
                CaseMatching::Smart,
                Normalization::Smart,
                AtomKind::Substring,
                false,
            ),
            negative: false,
            rule: Rule::Nucleo,
            scope: Scope::Names,
        }
    }

    /// The score of this word inside `text`, with the matched character indices, sorted, in
    /// `indices`.
    fn find(
        &self,
        text: &str,
        matcher: &mut Matcher,
        chars: &mut Vec<char>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        indices.clear();
        let haystack = Utf32Str::new(text, chars);
        let score = match (&self.rule, haystack) {
            (Rule::Cased { folded, needle }, Utf32Str::Ascii(bytes)) => {
                let score = folded.indices(haystack, matcher, indices)?;
                let matched = indices.iter().map(|&i| bytes.get(i as usize).copied());
                matched
                    .eq(needle.iter().copied().map(Some))
                    .then_some(score)
            }
            _ => self.atom.indices(haystack, matcher, indices),
        }?;
        indices.sort_unstable();
        if matches!(self.rule, Rule::WordStarts) {
            let mut previous = None;
            let starts = indices.iter().all(|&index| {
                let follows = previous.is_some_and(|previous| previous + 1 == index);
                previous = Some(index);
                follows || index == 0 || !haystack.get(index - 1).is_alphanumeric()
            });
            return starts.then_some(score);
        }
        Some(score)
    }
}

/// The query split at spaces, as nucleo splits it: `\ ` is a space inside a word.
fn tokens(query: &str) -> impl Iterator<Item = &str> {
    let mut escaped = false;
    query
        .split(move |c| {
            let split = c == ' ' && !escaped;
            escaped = c == '\\';
            split
        })
        .filter(|token| !token.is_empty())
}

/// Sorted character indices into `text` as byte ranges, neighbours joined.
fn ranges(text: &str, indices: &mut Vec<u32>) -> Vec<Range<usize>> {
    indices.sort_unstable();
    indices.dedup();
    let mut wanted = indices.iter().copied().peekable();
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for (position, (start, c)) in text.char_indices().enumerate() {
        let Some(next) = wanted.peek() else { break };
        if *next as usize != position {
            continue;
        }
        wanted.next();
        let end = start + c.len_utf8();
        match ranges.last_mut() {
            Some(last) if last.end == start => last.end = end,
            _ => ranges.push(start..end),
        }
    }
    ranges
}

impl Ranker {
    pub(crate) fn new(entries: Vec<Entry>, recents: &Recents) -> Ranker {
        Ranker {
            items: entries
                .into_iter()
                .map(|entry| Item::new(entry, recents))
                .collect(),
            matcher: Matcher::new(Config::DEFAULT),
            chars: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// An empty query lists used hosts first, the latest first, then the rest in file order. A
    /// query sorts by `(Reverse(score), Reverse(last use), shown_len, index)`.
    ///
    /// `Reverse(None)` sorts after every `Reverse(Some(_))`. With no history both keys are the
    /// keys this function had before, so the order is unchanged.
    pub(crate) fn rank(&mut self, query: &str) -> Vec<Match> {
        let words = self.words(query);
        if words.is_empty() {
            let mut order: Vec<usize> = (0..self.items.len()).collect();
            order.sort_by_key(|&item| self.items[item].last_at());
            return order
                .into_iter()
                .map(|item| Match {
                    item,
                    hits: Hits::default(),
                })
                .collect();
        }
        let mut found = Vec::new();
        for index in 0..self.items.len() {
            let Some((score, hits)) = self.score(index, &words) else {
                continue;
            };
            let item = &self.items[index];
            found.push((Reverse(score), item.last_at(), item.shown_len, index, hits));
        }
        found.sort_by_key(|&(score, last, shown_len, index, _)| (score, last, shown_len, index));
        found
            .into_iter()
            .map(|(_, _, _, item, hits)| Match { item, hits })
            .collect()
    }

    /// Each word looks in the name fields when some host's name fields match it, else in
    /// `HostName` and `User` when some host's match it. A plain word matches as a substring there;
    /// only when no field of any host contains it does it fall back to word starts in the names.
    fn words(&mut self, query: &str) -> Vec<Word> {
        let mut words = Word::parse(query);
        for word in &mut words {
            let substring = word.is_plain().then(|| word.substring());
            let probe = substring.as_ref().unwrap_or(word);
            let scope = [Scope::Names, Scope::Written].into_iter().find(|&scope| {
                self.items.iter().any(|item| {
                    item.fields(scope).any(|(_, text)| {
                        let (matcher, chars, indices) =
                            (&mut self.matcher, &mut self.chars, &mut self.indices);
                        probe.find(text, matcher, chars, indices).is_some()
                    })
                })
            });
            match (scope, substring) {
                (Some(scope), Some(substring)) => *word = Word { scope, ..substring },
                (Some(scope), None) => word.scope = scope,
                (None, Some(_)) => word.rule = Rule::WordStarts,
                (None, None) => {}
            }
        }
        words
    }

    /// Every positive word matches inside one field of its scope, and no negative word matches
    /// any. The score is the sum of each word's best field. The hits go to HOST whenever HOST
    /// matched, since the host name is what a person types, and otherwise to the best field.
    fn score(&mut self, index: usize, words: &[Word]) -> Option<(u32, Hits)> {
        let item = &self.items[index];
        let mut total = 0;
        let mut env = Vec::new();
        let mut host = Vec::new();
        let mut what = Vec::new();
        for word in words {
            let mut best: Option<(u16, Field, Vec<u32>)> = None;
            let mut in_host: Option<Vec<u32>> = None;
            for (field, text) in item.fields(word.scope) {
                let (matcher, chars, indices) =
                    (&mut self.matcher, &mut self.chars, &mut self.indices);
                let Some(score) = word.find(text, matcher, chars, indices) else {
                    continue;
                };
                if word.negative {
                    return None;
                }
                if field == Field::Host {
                    in_host = Some(self.indices.clone());
                }
                if best.as_ref().is_none_or(|(best, _, _)| score > *best) {
                    best = Some((score, field, self.indices.clone()));
                }
            }
            if word.negative {
                continue;
            }
            let (score, field, indices) = best?;
            total += u32::from(score);
            let (field, indices) = in_host.map_or((field, indices), |host| (Field::Host, host));
            match field {
                Field::Env => env.extend(indices),
                Field::Host => host.extend(indices),
                Field::What => what.extend(indices),
                Field::Alias | Field::Written => {}
            }
        }
        let entry = &item.entry;
        let hits = Hits {
            env: ranges(&entry.env, &mut env),
            host: ranges(entry.host.as_str(), &mut host),
            what: ranges(&entry.what, &mut what),
        };
        Some((total, hits))
    }

    pub(crate) fn entry(&self, found: &Match) -> &Entry {
        &self.items[found.item].entry
    }

    /// `2h`, or `""` for a host never connected to.
    pub(crate) fn ago(&self, found: &Match) -> &str {
        self.items[found.item]
            .last
            .as_ref()
            .map_or("", |last| last.ago.as_str())
    }

    /// Some host, matched or not, has a last use.
    pub(crate) fn remembers(&self) -> bool {
        self.items.iter().any(|item| item.last.is_some())
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use test_case::test_case;

    use super::*;
    use crate::config::Host;

    fn entry(env: &str, host: &str, what: &str, aliases: &[&str]) -> Entry {
        Entry {
            env: env.to_owned(),
            host: Host::literal(host).unwrap(),
            what: what.to_owned(),
            aliases: aliases.iter().map(|alias| alias.to_string()).collect(),
            hostname: None,
            user: None,
        }
    }

    fn written(env: &str, host: &str, what: &str, hostname: &str, user: &str) -> Entry {
        let text = |value: &str| (!value.is_empty()).then(|| value.to_owned());
        Entry {
            hostname: text(hostname),
            user: text(user),
            ..entry(env, host, what, &[])
        }
    }

    /// Look-alike hosts in five environments, as on a server with one file per environment.
    fn fleet() -> Vec<Entry> {
        vec![
            written(
                "PRODUCTION",
                "prod-bastion",
                "jump host",
                "10.13.0.1",
                "ubuntu",
            ),
            written("PRODUCTION", "prod-db-primary", "", "10.13.1.9", "ubuntu"),
            written(
                "PRODUCTION",
                "prod-db-replica",
                "",
                "10.13.1.10",
                "postgres",
            ),
            written("PRODUCTION", "prod-k8s-worker1", "", "10.13.1.21", ""),
            written("PRODUCTION", "prod-consul1", "", "10.13.1.31", ""),
            written("PRODUCTION", "prod-consul2", "", "10.13.1.32", ""),
            written("PRODUCTION", "prod-gitlab-runner", "", "10.13.1.41", ""),
            written("PRODUCTION", "api-prod", "public API", "", ""),
            written("NEW STAGING", "newstg-bastion", "", "10.12.0.1", "ubuntu"),
            written("NEW STAGING", "newstg-consul1", "", "", ""),
            written("NEW STAGING", "newstg-db-primary", "", "", ""),
            written("NEW STAGING", "newstg-web1", "web frontend", "", ""),
            written("OLD STAGING", "oldstg-consul1", "", "", ""),
            written("OLD STAGING", "oldstg-k8s-worker1", "", "", ""),
            written("LAB", "lab-db-primary", "", "", ""),
            written("LAB", "lab-k8s-worker1", "ทดสอบ เซิร์ฟเวอร์", "", ""),
            written("DEV", "dev-bastion", "", "", "deploy"),
            written("DEV", "dev-gitlab-runner", "", "", ""),
        ]
    }

    #[test_case("git" => "dev-gitlab-runner prod-gitlab-runner"; "letters are not picked from several columns")]
    #[test_case("ru" => "dev-gitlab-runner prod-gitlab-runner"; "the ENV column does not lend letters")]
    #[test_case("web" => "newstg-web1"; "a word some host contains matches only as a substring")]
    #[test_case("db" => "lab-db-primary newstg-db-primary prod-db-primary prod-db-replica"; "no sparse letters inside one column")]
    #[test_case("prod db" => "prod-db-primary prod-db-replica"; "two words")]
    #[test_case("lab db" => "lab-db-primary"; "lab in gitlab does not need db")]
    #[test_case("pdb" => "prod-db-primary prod-db-replica"; "no host contains it so the letters start words")]
    #[test_case("pk8s" => "prod-k8s-worker1"; "word starts then a run")]
    #[test_case("^prod-db" => "prod-db-primary prod-db-replica"; "a caret anchors the start of the host")]
    #[test_case("^PROD" => "api-prod prod-bastion prod-consul1 prod-consul2 prod-db-primary prod-db-replica prod-gitlab-runner prod-k8s-worker1"; "a capital caret word is case sensitive and anchors ENV")]
    #[test_case("prod$" => "api-prod"; "a dollar anchors the end of the host")]
    #[test_case("API$" => "api-prod"; "a capital dollar word anchors the end of WHAT")]
    #[test_case("10.13.1.9" => "prod-db-primary"; "the address written under the host")]
    #[test_case("10.13" => "prod-bastion prod-consul1 prod-consul2 prod-db-primary prod-db-replica prod-gitlab-runner prod-k8s-worker1"; "part of an address")]
    #[test_case("ubuntu" => "newstg-bastion prod-bastion prod-db-primary"; "the user written under the host")]
    #[test_case("ubuntu@10.13.1.9" => "prod-db-primary"; "user at address from shell history")]
    #[test_case("deploy@dev-bastion" => "dev-bastion"; "user at host")]
    #[test_case("postgres" => "prod-db-replica"; "a whole user")]
    #[test_case("1.10" => "prod-db-replica"; "the end of an address")]
    #[test_case("db !stg" => "lab-db-primary prod-db-primary prod-db-replica"; "a negated word excludes")]
    #[test_case("db !10.13" => "lab-db-primary newstg-db-primary"; "a negated address excludes")]
    #[test_case("'consul1" => "newstg-consul1 oldstg-consul1 prod-consul1"; "a quoted word")]
    #[test_case("stg" => "newstg-bastion newstg-consul1 newstg-db-primary newstg-web1 oldstg-consul1 oldstg-k8s-worker1"; "stg")]
    #[test_case("consul 1" => "newstg-consul1 oldstg-consul1 prod-consul1"; "a 1 in a name keeps the 1 in an address out")]
    #[test_case("worker1" => "lab-k8s-worker1 oldstg-k8s-worker1 prod-k8s-worker1"; "worker1")]
    #[test_case("k8s worker" => "lab-k8s-worker1 oldstg-k8s-worker1 prod-k8s-worker1"; "k8s and worker")]
    #[test_case("staging" => "newstg-bastion newstg-consul1 newstg-db-primary newstg-web1 oldstg-consul1 oldstg-k8s-worker1"; "a word of ENV")]
    #[test_case("เซิร์ฟ" => "lab-k8s-worker1"; "thai")]
    fn a_word_matches_inside_one_field(query: &str) -> String {
        let mut ranker = with_no_history(fleet());
        let mut found = hosts(&mut ranker, query);
        found.sort();
        found.join(" ")
    }

    /// ENV, HOST and WHAT hits of `host` under `query`.
    fn hits_of(query: &str, host: &str) -> [Vec<(usize, usize)>; 3] {
        let mut ranker = with_no_history(fleet());
        let found = ranker.rank(query);
        let Some(m) = found.iter().find(|m| ranker.entry(m).host.as_str() == host) else {
            panic!("{query:?} does not match {host}");
        };
        [&m.hits.env, &m.hits.host, &m.hits.what].map(|hits| spans(hits))
    }

    #[test]
    fn hits_land_in_the_field_the_word_matched() {
        assert_eq!(hits_of("^PROD", "api-prod"), [vec![(0, 4)], vec![], vec![]]);
        assert_eq!(hits_of("API$", "api-prod"), [vec![], vec![], vec![(7, 10)]]);
        assert_eq!(
            hits_of("pdb", "prod-db-primary"),
            [vec![], vec![(0, 1), (5, 7)], vec![]]
        );
        assert_eq!(
            hits_of("ubuntu@10.13.1.9", "prod-db-primary"),
            [vec![], vec![], vec![]]
        );
    }

    fn with_no_history(entries: Vec<Entry>) -> Ranker {
        Ranker::new(entries, &Recents::none())
    }

    fn spans(ranges: &[Range<usize>]) -> Vec<(usize, usize)> {
        ranges.iter().map(|r| (r.start, r.end)).collect()
    }

    fn hosts(ranker: &mut Ranker, query: &str) -> Vec<String> {
        let found = ranker.rank(query);
        found
            .iter()
            .map(|m| ranker.entry(m).host.to_string())
            .collect()
    }

    #[test]
    fn empty_query_keeps_file_order() {
        let mut ranker = with_no_history(vec![
            entry("E", "zz", "", &[]),
            entry("E", "aa", "", &[]),
            entry("", "mm", "x", &["m2"]),
        ]);
        for query in ["", "   "] {
            assert_eq!(hosts(&mut ranker, query), ["zz", "aa", "mm"]);
            let found = ranker.rank(query);
            assert!(found.iter().all(|m| {
                [&m.hits.env, &m.hits.host, &m.hits.what]
                    .iter()
                    .all(|hit| hit.is_empty())
            }));
        }
    }

    #[test]
    fn ties_break_on_shown_length_then_file_order() {
        let mut ranker = with_no_history(vec![
            entry("E", "web1", "a much longer description", &[]),
            entry(
                "E",
                "web2",
                "short",
                &["a", "very", "long", "alias", "list", "for", "web2"],
            ),
            entry("E", "web3", "short", &[]),
        ]);
        assert_eq!(hosts(&mut ranker, "web"), ["web2", "web3", "web1"]);
    }

    #[test]
    fn alias_matches_but_highlights_nothing_hidden() {
        let mut ranker = with_no_history(vec![
            entry("OLD STAGING", "oldstg-bastion", "jump", &["stg-haproxy"]),
            entry("OLD STAGING", "oldstg-db", "", &[]),
        ]);
        let found = ranker.rank("'haproxy");
        assert_eq!(found.len(), 1);
        assert_eq!(ranker.entry(&found[0]).host.as_str(), "oldstg-bastion");
        let hits = &found[0].hits;
        assert!(hits.env.is_empty() && hits.host.is_empty() && hits.what.is_empty());
    }

    #[test]
    fn a_word_the_host_name_holds_is_highlighted_there() {
        let mut ranker = with_no_history(vec![entry(
            "PRODUCTION",
            "prod-db-primary",
            "db-primary",
            &[],
        )]);
        let found = ranker.rank("prod db");
        let hits = &found[0].hits;
        assert_eq!(spans(&hits.host), [(0, 4), (5, 7)]);
        assert!(hits.env.is_empty() && hits.what.is_empty());
    }

    #[test]
    fn hits_are_byte_ranges_per_column() {
        let mut ranker = with_no_history(vec![entry("E", "h", "a \u{2014} b", &[])]);
        let found = ranker.rank("\u{2014}");
        let hits = &found[0].hits;
        assert_eq!(spans(&hits.what), [(2, 5)]);
        assert!(hits.env.is_empty() && hits.host.is_empty());

        let mut ranker = with_no_history(vec![entry("LAB", "web-a", "web VMs", &[])]);
        let found = ranker.rank("^lab 'b- vm");
        let hits = &found[0].hits;
        assert_eq!(spans(&hits.env), [(0, 3)]);
        assert_eq!(spans(&hits.host), [(2, 4)]);
        assert_eq!(spans(&hits.what), [(4, 6)]);
    }

    #[test]
    fn thai_combining_marks_match_and_highlight_as_written() {
        let what = "ทดสอบ เซิร์ฟเวอร์";
        let query = "เซิร์ฟ";
        let mut ranker = with_no_history(vec![entry("E", "h", what, &[])]);
        let found = ranker.rank(query);
        assert_eq!(found.len(), 1, "a query with Thai combining marks matches");
        let start = what.find(query).unwrap();
        assert_eq!(spans(&found[0].hits.what), [(start, start + query.len())]);
    }

    #[test]
    fn smart_case_and_extended_syntax() {
        let mut ranker = with_no_history(vec![
            entry("NEW STAGING", "newstg-consul1", "consul server 1", &[]),
            entry(
                "NEW STAGING",
                "newstg-consul-ui",
                "consul web UI tunnel",
                &[],
            ),
            entry("OLD STAGING", "oldstg-consul1", "", &[]),
            entry("LAB", "devstack", "", &[]),
            entry("NEW STAGING", "lab-net", "", &[]),
        ]);
        let mut lower = hosts(&mut ranker, "lab");
        lower.sort();
        assert_eq!(lower, ["devstack", "lab-net"]);
        assert_eq!(hosts(&mut ranker, "Lab"), Vec::<String>::new());
        assert_eq!(hosts(&mut ranker, "LAB"), ["devstack"]);
        assert_eq!(hosts(&mut ranker, "^new 'consul !ui"), ["newstg-consul1"]);
        assert_eq!(hosts(&mut ranker, "1$ oldstg"), ["oldstg-consul1"]);
    }

    const NOW: u64 = 1_794_556_800;

    fn used(names: &[(&str, u64)]) -> Recents {
        let at = |seconds: u64| UNIX_EPOCH + Duration::from_secs(seconds);
        Recents::of(
            names.iter().map(|&(name, ago)| (name, at(NOW - ago))),
            at(NOW),
        )
    }

    fn stagings() -> Vec<Entry> {
        vec![
            entry("NEW STAGING", "newstg-consul1", "", &[]),
            entry("NEW STAGING", "newstg-consul2", "", &[]),
            entry("OLD STAGING", "oldstg-consul1", "", &[]),
            entry("PRODUCTION", "prod-consul1", "", &[]),
        ]
    }

    #[test]
    fn an_empty_query_lists_used_hosts_latest_first_then_file_order() {
        let recents = used(&[("prod-consul1", 60), ("newstg-consul2", 7200)]);
        let mut ranker = Ranker::new(stagings(), &recents);
        for query in ["", "   "] {
            assert_eq!(
                hosts(&mut ranker, query),
                [
                    "prod-consul1",
                    "newstg-consul2",
                    "newstg-consul1",
                    "oldstg-consul1"
                ]
            );
        }
    }

    #[test]
    fn hosts_used_in_the_same_second_keep_file_order() {
        let recents = used(&[("oldstg-consul1", 5), ("newstg-consul2", 5)]);
        let mut ranker = Ranker::new(stagings(), &recents);
        assert_eq!(
            hosts(&mut ranker, "")[..2],
            ["newstg-consul2", "oldstg-consul1"]
        );
    }

    #[test]
    fn equal_scores_list_the_host_used_last_before_a_shorter_row() {
        let entries = vec![
            entry("E", "web1", "a much longer description", &[]),
            entry("E", "web2", "short", &[]),
            entry("E", "web3", "short", &[]),
            entry("E", "web4", "a longer one", &[]),
        ];
        let mut never = Ranker::new(entries, &Recents::none());
        assert_eq!(hosts(&mut never, "web"), ["web2", "web3", "web4", "web1"]);

        let entries = vec![
            entry("E", "web1", "a much longer description", &[]),
            entry("E", "web2", "short", &[]),
            entry("E", "web3", "short", &[]),
            entry("E", "web4", "a longer one", &[]),
        ];
        let recents = used(&[("web1", 60), ("web4", 3600)]);
        let mut ranker = Ranker::new(entries, &recents);
        assert_eq!(
            hosts(&mut ranker, "web"),
            ["web1", "web4", "web2", "web3"],
            "used hosts, latest first, then shown length, then file order"
        );
    }

    #[test]
    fn consul_across_environments_opens_on_the_node_used_last() {
        let recents = used(&[("newstg-consul1", 7200), ("prod-consul1", 259_200)]);
        let mut ranker = Ranker::new(stagings(), &recents);
        assert_eq!(
            hosts(&mut ranker, "consul"),
            [
                "newstg-consul1",
                "prod-consul1",
                "newstg-consul2",
                "oldstg-consul1"
            ]
        );
    }

    #[test]
    fn a_better_score_beats_a_recent_host() {
        let entries = vec![
            entry("PRODUCTION", "prod-dev-bastion", "", &[]),
            entry("PRODUCTION", "prod-db-primary", "", &[]),
        ];
        let recents = used(&[("prod-dev-bastion", 60)]);
        let mut ranker = Ranker::new(entries, &recents);
        assert_eq!(
            hosts(&mut ranker, "pdb"),
            ["prod-db-primary", "prod-dev-bastion"],
            "d and b in a row beat three word starts, though the bastion was used a minute ago"
        );
    }

    #[test]
    fn history_never_changes_which_hosts_match() {
        let recents = used(&[("prod-consul1", 60), ("oldstg-consul1", 90)]);
        for query in ["", "consul", "^new", "'1 !prod", "zzz", "o"] {
            let mut with = Ranker::new(stagings(), &recents);
            let mut without = Ranker::new(stagings(), &Recents::none());
            let mut left = hosts(&mut with, query);
            let mut right = hosts(&mut without, query);
            left.sort();
            right.sort();
            assert_eq!(left, right, "{query:?}");
        }
    }

    #[test]
    fn the_age_is_text_per_match_and_remembers_says_whether_any_host_has_one() {
        let recents = used(&[("newstg-consul1", 7200)]);
        let mut ranker = Ranker::new(stagings(), &recents);
        assert!(ranker.remembers());
        let found = ranker.rank("consul");
        let ages: Vec<_> = found.iter().map(|m| ranker.ago(m)).collect();
        assert_eq!(ages, ["2h", "", "", ""]);

        let mut none = Ranker::new(stagings(), &Recents::none());
        assert!(!none.remembers());
        let found = none.rank("");
        assert!(found.iter().all(|m| none.ago(m).is_empty()));
    }
}
