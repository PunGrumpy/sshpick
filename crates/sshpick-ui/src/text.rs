use ratatui::text::Line;

pub(crate) const ELLIPSIS: &str = "...";
const CUT: &str = "…";

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

pub(crate) fn fit<'a>(runs: &[(&'a str, bool)], limit: usize) -> Vec<(&'a str, bool)> {
    if runs.iter().map(|&(text, _)| columns(text)).sum::<usize>() <= limit {
        return runs.to_vec();
    }
    let Some(budget) = limit.checked_sub(columns(CUT)) else {
        return Vec::new();
    };
    let mut kept = Vec::new();
    let mut used = 0;
    'runs: for &(text, matched) in runs {
        for (start, c) in text.char_indices() {
            let end = start + c.len_utf8();
            let cells = columns(&text[start..end]);
            if used + cells > budget {
                if start > 0 {
                    kept.push((&text[..start], matched));
                }
                break 'runs;
            }
            used += cells;
        }
        kept.push((text, matched));
    }
    kept.push((CUT, false));
    kept
}

pub(crate) fn truncate_to_width(text: &str, limit: usize) -> String {
    if columns(text) <= limit {
        return text.to_owned();
    }
    let dotted = limit > ELLIPSIS.len();
    let keep = if dotted {
        limit - ELLIPSIS.len()
    } else {
        limit
    };
    let mut cut = String::new();
    let mut used = 0;
    for c in text.chars() {
        used += columns(c.encode_utf8(&mut [0; 4]));
        if used > keep {
            break;
        }
        cut.push(c);
    }
    if dotted {
        cut.push_str(ELLIPSIS);
    }
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_cuts_on_display_width_and_keeps_runs_whole_or_cut_cleanly() {
        let plain = |text| [(text, false)];
        assert_eq!(fit(&plain("abcdef"), 6), plain("abcdef"));
        assert_eq!(fit(&plain("abcdef"), 5), [("abcd", false), (CUT, false)]);
        assert_eq!(fit(&plain("abcdef"), 1), [(CUT, false)]);
        assert!(fit(&plain("abcdef"), 0).is_empty());

        let runs = [("ab", false), ("cdef", true), ("gh", false)];
        assert_eq!(
            fit(&runs, 5),
            [("ab", false), ("cd", true), (CUT, false)],
            "a cut run keeps its match"
        );
        assert_eq!(fit(&runs, 3), [("ab", false), (CUT, false)]);
        assert_eq!(fit(&runs, 8), runs);
        assert_eq!(fit(&runs, 7), [("ab", false), ("cdef", true), (CUT, false)]);

        assert_eq!(
            fit(&plain("a — b"), 4),
            [("a —", false), (CUT, false)],
            "an em dash is one column and three bytes"
        );
        assert_eq!(
            fit(&plain("日本語です"), 6),
            [("日本", false), (CUT, false)],
            "a wide char that does not fit is dropped, not split"
        );
        assert_eq!(
            fit(&plain("日本語です"), 7),
            [("日本語", false), (CUT, false)]
        );
        assert_eq!(fit(&plain("日本語です"), 10), plain("日本語です"));
        assert_eq!(
            fit(&plain("สวัสดี"), 3),
            [("สวั", false), (CUT, false)],
            "a combining mark stays with its base"
        );
        assert_eq!(fit(&plain("สวัสดี"), 4), plain("สวัสดี"));
    }

    #[test]
    fn truncation_counts_display_columns() {
        assert_eq!(truncate_to_width("abcdef", 6), "abcdef");
        assert_eq!(truncate_to_width("abcdef", 5), "ab...");
        assert_eq!(truncate_to_width("abcdef", 3), "abc");
        assert_eq!(truncate_to_width("abcdef", 0), "");
        assert_eq!(truncate_to_width("日本語です", 7), "日本...");
        assert_eq!(truncate_to_width("日本語です", 10), "日本語です");
        assert_eq!(truncate_to_width("日本語です", 8), "日本...");
        assert_eq!(truncate_to_width("日本語です", 9), "日本語...");
    }
}
