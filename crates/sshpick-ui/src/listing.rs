use sshpick_core::{Entry, Run};

use crate::color::ColorConfig;
use crate::text::{columns, fit};

const INDENT: &str = "  ";
const GAP: &str = "  ";
const TITLES: [&str; 3] = ["ENV", "HOST", "WHAT"];

/// The hosts as an aligned table for a terminal: a 2-space indent, 2 spaces between columns and
/// a dim header. ENV disappears when no host has one. WHAT is cut with `…` to `width`, and no
/// line ends in a space.
pub fn hosts(entries: &[&Entry], width: usize, colors: ColorConfig) -> String {
    let widest = |text: fn(&Entry) -> &str| {
        entries
            .iter()
            .map(|entry| columns(text(entry)))
            .max()
            .unwrap_or(0)
    };
    let env = widest(|entry| &entry.env);
    let env = (env > 0).then(|| env.max(columns(TITLES[0])));
    let host = widest(|entry| entry.host.as_str()).max(columns(TITLES[1]));
    let taken = INDENT.len() + env.map_or(0, |env| env + GAP.len()) + host + GAP.len();
    let room = width.saturating_sub(taken);

    let row = |env_text: &str, host_text: &str, what: &str, dim_host: bool| {
        let mut line = String::from(INDENT);
        if let Some(env_width) = env {
            line.push_str(&colors.dim(&pad(env_text, env_width)));
            line.push_str(GAP);
        }
        let host_cell = if what.is_empty() {
            host_text.to_owned()
        } else {
            pad(host_text, host)
        };
        line.push_str(&if dim_host {
            colors.dim(&host_cell)
        } else {
            host_cell
        });
        if !what.is_empty() {
            line.push_str(GAP);
            line.push_str(&colors.dim(what));
        }
        line
    };

    let mut out = row(TITLES[0], TITLES[1], TITLES[2], true);
    out.push('\n');
    for entry in entries {
        let what: String = fit(
            &[Run {
                text: &entry.what,
                matched: false,
            }],
            room,
        )
        .iter()
        .map(|run| run.text)
        .collect();
        out.push_str(&row(&entry.env, entry.host.as_str(), &what, false));
        out.push('\n');
    }
    out
}

fn pad(text: &str, width: usize) -> String {
    let fill = width.saturating_sub(columns(text));
    format!("{text}{}", " ".repeat(fill))
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use sshpick_core::Picker;

    use super::*;

    const NONE: ColorConfig = ColorConfig::none();

    fn entries(home: &str) -> Vec<Entry> {
        let home = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(home);
        sshpick_core::load(&home).unwrap().entries
    }

    fn table(home: &str, query: &str, width: usize) -> String {
        let picker = Picker::new(entries(home), query);
        hosts(&picker.listing(), width, NONE)
    }

    #[test]
    fn a_table_aligns_the_columns_under_a_header() {
        assert_snapshot!("hosts_80", table("ui", "", 80));
    }

    #[test]
    fn a_narrow_terminal_cuts_what_and_keeps_the_host() {
        assert_snapshot!("hosts_40", table("ui", "", 40));
    }

    #[test]
    fn env_hides_when_no_host_has_one() {
        assert_snapshot!("hosts_flat", table("flat", "", 80));
    }

    #[test]
    fn a_query_keeps_the_pickers_order() {
        assert_snapshot!("hosts_consul", table("ui", "new consul", 80));
    }

    #[test]
    fn no_line_ends_in_a_space_at_any_width() {
        for width in 0..=120 {
            for line in table("ui", "", width).lines() {
                assert!(!line.ends_with(' '), "{width}: {line:?}");
            }
        }
    }

    #[test]
    fn colour_styles_the_header_env_and_what_and_leaves_the_host_plain() {
        let colors = ColorConfig::new(true, false, &|_| None);
        let picker = Picker::new(entries("ui"), "lab-a");
        let out = hosts(&picker.listing(), 80, colors);
        let mut lines = out.lines();
        let header = lines.next().unwrap();
        assert_eq!(
            header,
            "  \x1b[2mENV\x1b[22m  \x1b[2mHOST \x1b[22m  \x1b[2mWHAT\x1b[22m"
        );
        let row = lines.next().unwrap();
        assert_eq!(
            row,
            "  \x1b[2mLAB\x1b[22m  lab-a  \x1b[2mLAB — personal VMs\x1b[22m"
        );
    }
}
