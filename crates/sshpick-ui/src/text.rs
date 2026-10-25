use ratatui::text::Line;
use sshpick_core::{Entry, Run};

const CUT: &str = "…";

pub(crate) const TITLES: [&str; 3] = ["ENV", "HOST", "WHAT"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColumnWidths {
    pub(crate) env: Option<usize>,
    pub(crate) host: usize,
}

impl ColumnWidths {
    pub(crate) fn of<'a>(entries: impl IntoIterator<Item = &'a Entry>) -> ColumnWidths {
        let (mut env, mut host) = (0, 0);
        for entry in entries {
            env = env.max(columns(&entry.env));
            host = host.max(columns(entry.host.as_str()));
        }
        ColumnWidths {
            env: (env > 0).then(|| env.max(columns(TITLES[0]))),
            host: host.max(columns(TITLES[1])),
        }
    }
}

pub(crate) fn width(text: &str) -> u16 {
    u16::try_from(columns(text)).unwrap_or(u16::MAX)
}

pub(crate) fn columns(text: &str) -> usize {
    Line::from(text).width()
}

pub(crate) fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_owned()
    } else {
        format!("{noun}s")
    }
}

pub(crate) fn fit<'a>(runs: &[Run<'a>], limit: usize) -> Vec<Run<'a>> {
    let parts: Vec<_> = runs.iter().map(|run| (run.text, run.matched)).collect();
    fit_parts(&parts, limit, false)
        .into_iter()
        .map(|(text, matched)| Run { text, matched })
        .collect()
}

pub(crate) fn fit_parts<'a, T: Copy>(
    parts: &[(&'a str, T)],
    limit: usize,
    plain: T,
) -> Vec<(&'a str, T)> {
    if parts.iter().map(|(text, _)| columns(text)).sum::<usize>() <= limit {
        return parts.to_vec();
    }
    let Some(budget) = limit.checked_sub(columns(CUT)) else {
        return Vec::new();
    };
    let mut kept = Vec::new();
    let mut used = 0;
    'parts: for &(text, tag) in parts {
        for (start, c) in text.char_indices() {
            let end = start + c.len_utf8();
            let cells = columns(&text[start..end]);
            if used + cells > budget {
                if start > 0 {
                    kept.push((&text[..start], tag));
                }
                break 'parts;
            }
            used += cells;
        }
        kept.push((text, tag));
    }
    kept.push((CUT, plain));
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_hides_when_empty_and_titles_set_the_minimum() {
        assert_eq!(
            ColumnWidths::of([]),
            ColumnWidths { env: None, host: 4 },
            "HOST is never narrower than its title"
        );
    }

    #[test]
    fn fit_cuts_on_display_width_and_keeps_runs_whole_or_cut_cleanly() {
        let run = |text, matched| Run { text, matched };
        let plain = |text| [run(text, false)];
        assert_eq!(fit(&plain("abcdef"), 6), plain("abcdef"));
        assert_eq!(
            fit(&plain("abcdef"), 5),
            [run("abcd", false), run(CUT, false)]
        );
        assert_eq!(fit(&plain("abcdef"), 1), [run(CUT, false)]);
        assert!(fit(&plain("abcdef"), 0).is_empty());

        let runs = [run("ab", false), run("cdef", true), run("gh", false)];
        assert_eq!(
            fit(&runs, 5),
            [run("ab", false), run("cd", true), run(CUT, false)],
            "a cut run keeps its match"
        );
        assert_eq!(fit(&runs, 3), [run("ab", false), run(CUT, false)]);
        assert_eq!(fit(&runs, 8), runs);
        assert_eq!(
            fit(&runs, 7),
            [run("ab", false), run("cdef", true), run(CUT, false)]
        );

        assert_eq!(
            fit(&plain("a — b"), 4),
            [run("a —", false), run(CUT, false)],
            "an em dash is one column and three bytes"
        );
        assert_eq!(
            fit(&plain("日本語です"), 6),
            [run("日本", false), run(CUT, false)],
            "a wide char that does not fit is dropped, not split"
        );
        assert_eq!(
            fit(&plain("日本語です"), 7),
            [run("日本語", false), run(CUT, false)]
        );
        assert_eq!(fit(&plain("日本語です"), 10), plain("日本語です"));
        assert_eq!(
            fit(&plain("สวัสดี"), 3),
            [run("สวั", false), run(CUT, false)],
            "a combining mark stays with its base"
        );
        assert_eq!(fit(&plain("สวัสดี"), 4), plain("สวัสดี"));
    }

    #[test]
    fn fit_parts_keeps_each_parts_tag_and_ends_in_a_plain_cut() {
        let parts = [
            ("→", 1),
            (" ", 0),
            ("ubuntu@host", 0),
            (" via ", 1),
            ("bastion", 0),
        ];
        assert_eq!(fit_parts(&parts, 40, 0), parts);
        assert_eq!(
            fit_parts(&parts, 17, 0),
            [("→", 1), (" ", 0), ("ubuntu@host", 0), (" vi", 1), (CUT, 0)],
            "a cut part keeps its tag; the cut is plain"
        );
        assert_eq!(fit_parts(&parts, 3, 0), [("→", 1), (" ", 0), (CUT, 0)]);
        assert_eq!(fit_parts(&parts, 1, 0), [(CUT, 0)]);
        assert!(fit_parts(&parts, 0, 0).is_empty());
    }
}
