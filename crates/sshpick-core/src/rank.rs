use std::cmp::Reverse;
use std::ops::Range;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::config::Entry;

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
    fn new(entry: Entry) -> Item {
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
        }
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
    pub(crate) fn new(entries: Vec<Entry>) -> Ranker {
        Ranker {
            items: entries.into_iter().map(Item::new).collect(),
            matcher: Matcher::new(Config::DEFAULT),
            chars: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub(crate) fn rank(&mut self, query: &str) -> Vec<Match> {
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        if pattern.atoms.is_empty() {
            return (0..self.items.len())
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
            found.push((Reverse(score), item.shown_len, index, hits));
        }
        found.sort_by_key(|&(score, shown_len, index, _)| (score, shown_len, index));
        found
            .into_iter()
            .map(|(_, _, item, hits)| Match { item, hits })
            .collect()
    }

    pub(crate) fn entry(&self, found: &Match) -> &Entry {
        &self.items[found.item].entry
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
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
        let mut ranker = Ranker::new(vec![
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
        let mut ranker = Ranker::new(vec![
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
        let mut ranker = Ranker::new(vec![
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
        let mut ranker = Ranker::new(vec![entry("E", "h", "a \u{2014} b", &[])]);
        let found = ranker.rank("\u{2014}");
        let hits = &found[0].hits;
        assert_eq!(spans(&hits.what), [(2, 5)]);
        assert!(hits.env.is_empty() && hits.host.is_empty());

        let mut ranker = Ranker::new(vec![entry("LAB", "lab-a", "lab VMs", &[])]);
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
        let mut ranker = Ranker::new(vec![entry("E", "h", what, &[])]);
        let found = ranker.rank(query);
        assert_eq!(found.len(), 1, "a query with Thai combining marks matches");
        let start = what.find(query).unwrap();
        assert_eq!(spans(&found[0].hits.what), [(start, start + query.len())]);
    }

    #[test]
    fn smart_case_and_extended_syntax() {
        let mut ranker = Ranker::new(vec![
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
}
