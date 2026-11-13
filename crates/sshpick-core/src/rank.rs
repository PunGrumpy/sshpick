use std::cmp::Reverse;
use std::ops::Range;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
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
    haystack: String,
    columns: Columns,
    shown_len: usize,
    /// Joined once, when the ranker is built. Ranking and drawing never look a name up again.
    last: Option<Last>,
}

#[derive(Debug)]
struct Columns {
    env: Range<usize>,
    host: Range<usize>,
    what: Range<usize>,
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
        let host = entry.host.as_str();
        let env = 0..entry.env.len();
        let host_range = env.end + 1..env.end + 1 + host.len();
        let what = host_range.end + 1..host_range.end + 1 + entry.what.len();
        let haystack = format!(
            "{}\t{host}\t{}\t{}",
            entry.env,
            entry.what,
            entry.aliases.join(" ")
        );
        let shown_len = haystack[..what.end].trim().chars().count();
        Item {
            entry,
            haystack,
            columns: Columns {
                env,
                host: host_range,
                what,
            },
            shown_len,
            last,
        }
    }

    fn last_at(&self) -> Reverse<Option<u64>> {
        Reverse(self.last.as_ref().map(|last| last.at))
    }

    fn hits(&self, indices: &mut Vec<u32>) -> Hits {
        indices.sort_unstable();
        indices.dedup();
        let mut wanted = indices.iter().copied().peekable();
        let mut hits = Hits::default();
        for (position, (start, c)) in self.haystack.char_indices().enumerate() {
            let Some(next) = wanted.peek() else { break };
            if *next as usize != position {
                continue;
            }
            wanted.next();
            let end = start + c.len_utf8();
            let columns = &self.columns;
            for (column, hit) in [
                (&columns.env, &mut hits.env),
                (&columns.host, &mut hits.host),
                (&columns.what, &mut hits.what),
            ] {
                if !column.contains(&start) {
                    continue;
                }
                let range = start - column.start..end - column.start;
                match hit.last_mut() {
                    Some(last) if last.end == range.start => last.end = range.end,
                    _ => hit.push(range),
                }
            }
        }
        hits
    }
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
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        if pattern.atoms.is_empty() {
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
        for (index, item) in self.items.iter().enumerate() {
            self.indices.clear();
            let haystack = Utf32Str::new(&item.haystack, &mut self.chars);
            let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut self.indices)
            else {
                continue;
            };
            let hits = item.hits(&mut self.indices);
            found.push((Reverse(score), item.last_at(), item.shown_len, index, hits));
        }
        found.sort_by_key(|&(score, last, shown_len, index, _)| (score, last, shown_len, index));
        found
            .into_iter()
            .map(|(_, _, _, item, hits)| Match { item, hits })
            .collect()
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

    use super::*;
    use crate::config::Host;

    fn entry(env: &str, host: &str, what: &str, aliases: &[&str]) -> Entry {
        Entry {
            env: env.to_owned(),
            host: Host::literal(host).unwrap(),
            what: what.to_owned(),
            aliases: aliases.iter().map(|alias| alias.to_string()).collect(),
        }
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
    fn hits_are_byte_ranges_per_column() {
        let mut ranker = with_no_history(vec![entry("E", "h", "a \u{2014} b", &[])]);
        let found = ranker.rank("\u{2014}");
        let hits = &found[0].hits;
        assert_eq!(spans(&hits.what), [(2, 5)]);
        assert!(hits.env.is_empty() && hits.host.is_empty());

        let mut ranker = with_no_history(vec![entry("LAB", "lab-a", "lab VMs", &[])]);
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
            entry("PRODUCTION", "prod-bastion", "jump host", &[]),
            entry("PRODUCTION", "prod-db-primary", "", &[]),
        ];
        let recents = used(&[("prod-bastion", 60)]);
        let mut ranker = Ranker::new(entries, &recents);
        assert_eq!(
            hosts(&mut ranker, "prod db"),
            ["prod-db-primary", "prod-bastion"],
            "bastion matches only as noise, and it is the one used a minute ago"
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
