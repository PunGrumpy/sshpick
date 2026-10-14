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

pub(crate) fn run(base: &CommandBase, query: &str) -> Result<u8, cli::Error> {
    let loaded = base.load()?;
    base.warn_all(&loaded.warnings);
    let picker = Picker::new(loaded.entries, query);
    let rows = picker.listing();
    if rows.is_empty() {
        return Ok(Exit::NoMatch.code());
    }
    write_rows(base.output.result(), &rows).map_err(Error::Write)?;
    Ok(Exit::Success.code())
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
    fn a_tab_in_a_comment_becomes_a_space() {
        assert_eq!(field("one\ttwo"), "one two");
        assert_eq!(field("plain"), "plain");
    }
}
