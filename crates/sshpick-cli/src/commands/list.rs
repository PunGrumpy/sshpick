use std::io::{self, Write};

use serde::Serialize;
use sshpick_core::{Entry, Picker, Warning};

use super::{CommandBase, HostView, write_json};
use crate::cli;
use crate::cli::Exit;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("cannot write the host list: {0}")]
    Write(#[source] io::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListFormat {
    Auto,
    Tsv,
    Json,
}

pub(crate) struct Request<'a> {
    pub(crate) query: &'a str,
    pub(crate) format: ListFormat,
    pub(crate) typed: Option<&'a str>,
}

#[derive(Serialize)]
struct View<'a> {
    query: &'a str,
    total: usize,
    hosts: Vec<HostView<'a>>,
    warnings: Vec<WarningView>,
}

#[derive(Serialize)]
struct WarningView {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    message: String,
}

impl WarningView {
    fn of(warning: &Warning) -> WarningView {
        let (kind, path) = match warning {
            Warning::Unreadable { path, .. } => ("unreadable", Some(path)),
            Warning::TooDeep { path } => ("too_deep", Some(path)),
            Warning::Cycle { path } => ("cycle", Some(path)),
            Warning::BadInclude { .. } => ("bad_include", None),
        };
        WarningView {
            kind,
            path: path.map(|path| path.display().to_string()),
            message: warning.to_string(),
        }
    }
}

pub(crate) fn run(base: &CommandBase, request: &Request) -> Result<u8, cli::Error> {
    let loaded = base.load()?;
    base.warn_all(&loaded.warnings);
    let Request {
        query,
        format,
        typed,
    } = *request;
    let human = format == ListFormat::Auto;
    if let Some(word) = typed.filter(|_| human) {
        base.note_if_also_a_host(word, &loaded.entries);
    }
    let total = loaded.entries.len();
    let picker = Picker::new(loaded.entries, query);
    let rows = picker.listing();
    let output = base.output;
    if human {
        output.log(&summary(rows.len(), total, query));
    }
    let code = if rows.is_empty() {
        Exit::NoMatch
    } else {
        Exit::Success
    };
    if format == ListFormat::Json {
        let view = View {
            query,
            total,
            hosts: rows.iter().map(|entry| HostView::of(entry)).collect(),
            warnings: loaded.warnings.iter().map(WarningView::of).collect(),
        };
        write_json(output.result(), &view).map_err(Error::Write)?;
        return Ok(code.code());
    }
    if rows.is_empty() {
        return Ok(code.code());
    }
    if human && output.human() {
        let table = sshpick_ui::listing::hosts(&rows, output.columns(), output.stdout_colors());
        output.result().write_all(table.as_bytes())
    } else {
        write_rows(output.result(), &rows)
    }
    .map_err(Error::Write)?;
    Ok(code.code())
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
        let (env, what) = (tsv_cell(&entry.env), tsv_cell(&entry.what));
        writeln!(out, "{env}\t{}\t{what}", entry.host)?;
    }
    out.flush()
}

fn tsv_cell(text: &str) -> String {
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
        assert_eq!(tsv_cell("one\ttwo"), "one two");
        assert_eq!(tsv_cell("plain"), "plain");
    }

    #[test]
    fn each_warning_has_a_stable_kind_and_a_path_when_it_has_one() {
        let path = std::path::PathBuf::from("/h/.ssh/a.conf");
        let cycle = WarningView::of(&Warning::Cycle { path: path.clone() });
        assert_eq!(
            (cycle.kind, cycle.path.as_deref()),
            ("cycle", Some("/h/.ssh/a.conf"))
        );
        assert_eq!(cycle.message, "/h/.ssh/a.conf: skipped, include cycle");
        let deep = WarningView::of(&Warning::TooDeep { path });
        assert_eq!(deep.kind, "too_deep");
        let bad = WarningView::of(&Warning::BadInclude {
            arg: "${X}".to_owned(),
            reason: "${X} is not set".to_owned(),
        });
        assert_eq!((bad.kind, bad.path), ("bad_include", None));
        let unreadable = WarningView::of(&Warning::Unreadable {
            path: std::path::PathBuf::from("/h/.ssh/b.conf"),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        });
        assert_eq!(
            (unreadable.kind, unreadable.path.as_deref()),
            ("unreadable", Some("/h/.ssh/b.conf"))
        );
    }
}
