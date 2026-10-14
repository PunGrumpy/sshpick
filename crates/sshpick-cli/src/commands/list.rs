use std::io::{self, Write};

use sshpick_core::{Entry, Picker};

use super::CommandBase;
use crate::cli;
use crate::cli::Exit;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("cannot write the host list: {0}")]
    Write(#[source] io::Error),
}

pub(crate) fn run(base: &CommandBase, query: &str, machine: bool) -> Result<u8, cli::Error> {
    let loaded = base.load()?;
    base.warn_all(&loaded.warnings);
    let total = loaded.entries.len();
    let picker = Picker::new(loaded.entries, query);
    let rows = picker.listing();
    let output = base.output;
    if !machine {
        output.log(&summary(rows.len(), total, query));
    }
    if rows.is_empty() {
        return Ok(Exit::NoMatch.code());
    }
    if machine || !output.human() {
        write_rows(output.result(), &rows)
    } else {
        let table = sshpick_ui::listing::hosts(&rows, output.columns(), output.stdout_colors());
        output.result().write_all(table.as_bytes())
    }
    .map_err(Error::Write)?;
    Ok(Exit::Success.code())
}

fn summary(matched: usize, total: usize, query: &str) -> String {
    let hosts = if total == 1 { "host" } else { "hosts" };
    if query.trim().is_empty() {
        return format!("{total} {hosts}");
    }
    let verb = if matched == 1 { "matches" } else { "match" };
    format!("{matched} of {total} {hosts} {verb} \"{query}\"")
}

fn write_rows(out: impl Write, rows: &[&Entry]) -> io::Result<()> {
    let mut out = io::BufWriter::new(out);
    for entry in rows {
        let (env, what) = (field(&entry.env), field(&entry.what));
        writeln!(out, "{env}\t{}\t{what}", entry.host)?;
    }
    out.flush()
}

/// A tab inside a comment would add a column and break `cut -f2`.
fn field(text: &str) -> String {
    text.replace('\t', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_counts_matches_against_all_hosts() {
        assert_eq!(summary(12, 12, ""), "12 hosts");
        assert_eq!(summary(1, 1, "  "), "1 host");
        assert_eq!(summary(4, 12, "consul"), "4 of 12 hosts match \"consul\"");
        assert_eq!(summary(1, 12, "a b"), "1 of 12 hosts matches \"a b\"");
        assert_eq!(summary(0, 12, "zzz"), "0 of 12 hosts match \"zzz\"");
    }

    #[test]
    fn a_tab_in_a_comment_becomes_a_space() {
        assert_eq!(field("one\ttwo"), "one two");
        assert_eq!(field("plain"), "plain");
    }
}
