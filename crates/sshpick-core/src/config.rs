use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::label;

/// ssh's include limit; the root file is depth 0.
const MAX_DEPTH: usize = 16;

/// A literal `Host` token: the alias you type, and the only thing sshpick ever passes to `ssh`.
///
/// Invariant: non-empty, no whitespace, no `*` or `?`, not negated with `!`. Only the config
/// walker builds one, so free text such as the query can never reach `ssh -G` (which "resolves"
/// any string to defaults and exits 0) or `exec`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Host(String);

#[derive(Debug)]
pub struct Entry {
    /// ENV column: the label of the included file the host is in (see `label::env_label`), or
    /// `""` for hosts written directly in `~/.ssh/config`.
    pub env: String,
    /// HOST column: the first literal on the `Host` line that no earlier line listed.
    pub host: Host,
    /// WHAT column: the `#` comment on the line directly above the `Host` line, else `""`.
    pub what: String,
    pub(crate) aliases: Vec<String>,
}

#[derive(Debug)]
pub struct Loaded {
    /// Rows in ssh's read order: lines in file order, each `Include` expanded in place, glob
    /// matches in byte order (`10.conf` before `9.conf`). A literal listed twice is one row.
    pub entries: Vec<Entry>,
    /// What cost rows without stopping the run.
    pub warnings: Vec<Warning>,
}

#[derive(Debug, thiserror::Error)]
#[error("cannot read {}: {source}", .path.display())]
pub struct LoadError {
    path: PathBuf,
    source: io::Error,
}

#[derive(Debug, thiserror::Error)]
pub enum Warning {
    #[error("{}: skipped, {source}", .path.display())]
    Unreadable { path: PathBuf, source: io::Error },
    #[error("{}: skipped, Include nested deeper than {MAX_DEPTH} levels", .path.display())]
    TooDeep { path: PathBuf },
    #[error("{}: skipped, include cycle", .path.display())]
    Cycle { path: PathBuf },
    #[error("Include {arg}: skipped, {reason}")]
    BadInclude { arg: String, reason: String },
}

/// Read `<home>/.ssh/config` and every file it includes, in ssh's order. Errs only when the root
/// file is unreadable.
pub fn load(home: &Path) -> Result<Loaded, LoadError> {
    Walk::new(home, &|name| std::env::var_os(name)).root()
}

impl Entry {
    /// Every name on the `Host` line that this row stands for: the one shown first, then the
    /// other literals, which are searchable and also reach the same host.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.host.as_str()).chain(self.aliases.iter().map(String::as_str))
    }
}

impl Host {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn literal(token: &str) -> Option<Host> {
        let literal = !token.is_empty()
            && !token.starts_with('!')
            && !token.contains(['*', '?'])
            && !token.contains(char::is_whitespace);
        literal.then(|| Host(token.to_owned()))
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

struct Walk<'a> {
    home: &'a Path,
    var: &'a dyn Fn(&str) -> Option<OsString>,
    seen: HashSet<String>,
    chain: Vec<PathBuf>,
    out: Loaded,
}

impl<'a> Walk<'a> {
    fn new(home: &'a Path, var: &'a dyn Fn(&str) -> Option<OsString>) -> Walk<'a> {
        Walk {
            home,
            var,
            seen: HashSet::new(),
            chain: Vec::new(),
            out: Loaded {
                entries: Vec::new(),
                warnings: Vec::new(),
            },
        }
    }

    fn root(mut self) -> Result<Loaded, LoadError> {
        let path = self.home.join(".ssh").join("config");
        let text = match read(&path) {
            Ok(text) => text,
            Err(source) => return Err(LoadError { path, source }),
        };
        self.lines(&path, &text, "", 0);
        Ok(self.out)
    }

    fn included(&mut self, path: &Path, depth: usize) {
        if depth > MAX_DEPTH {
            let path = path.to_path_buf();
            self.out.warnings.push(Warning::TooDeep { path });
            return;
        }
        if self.chain.iter().any(|open| open == path) {
            let path = path.to_path_buf();
            self.out.warnings.push(Warning::Cycle { path });
            return;
        }
        let text = match read(path) {
            Ok(text) => text,
            Err(source) => {
                let path = path.to_path_buf();
                self.out.warnings.push(Warning::Unreadable { path, source });
                return;
            }
        };
        let env = label::env_label(path, text.lines().next().unwrap_or_default());
        self.lines(path, &text, &env, depth);
    }

    fn lines(&mut self, path: &Path, text: &str, env: &str, depth: usize) {
        self.chain.push(path.to_path_buf());
        let mut what = "";
        for line in text.lines() {
            match classify(line) {
                Line::Comment(comment) => {
                    what = comment;
                    continue;
                }
                Line::Include(args) => {
                    for arg in &args {
                        for file in self.expand(arg) {
                            self.included(&file, depth + 1);
                        }
                    }
                }
                Line::Host(tokens) => self.host(&tokens, what, env),
                Line::Other => {}
            }
            what = "";
        }
        self.chain.pop();
    }

    fn expand(&mut self, arg: &str) -> Vec<PathBuf> {
        let Some(pattern) = self.pattern(arg) else {
            return Vec::new();
        };
        let options = glob::MatchOptions {
            require_literal_leading_dot: true,
            ..glob::MatchOptions::new()
        };
        let paths = match glob::glob_with(&pattern, options) {
            Ok(paths) => paths,
            Err(error) => {
                let (arg, reason) = (arg.to_owned(), error.to_string());
                self.out.warnings.push(Warning::BadInclude { arg, reason });
                return Vec::new();
            }
        };
        let mut files = Vec::new();
        for found in paths {
            match found {
                Ok(path) if path.is_file() => files.push(path),
                Ok(_) => {}
                Err(error) => {
                    let path = error.path().to_path_buf();
                    let source = io::Error::from(error);
                    self.out.warnings.push(Warning::Unreadable { path, source });
                }
            }
        }
        files
    }

    /// ssh resolves a relative `Include` against `~/.ssh`, never the including file's directory.
    fn pattern(&mut self, arg: &str) -> Option<String> {
        let home = glob::Pattern::escape(&self.home.to_string_lossy());
        let mut rest = arg;
        let mut pattern = String::new();
        if let Some(after) = rest.strip_prefix('~')
            && (after.is_empty() || after.starts_with('/'))
        {
            pattern.push_str(&home);
            rest = after;
        }
        while let Some(c) = rest.chars().next() {
            if let Some(after) = rest.strip_prefix("%%") {
                pattern.push('%');
                rest = after;
            } else if let Some(after) = rest.strip_prefix("%d") {
                pattern.push_str(&home);
                rest = after;
            } else if let Some(after) = rest.strip_prefix('%') {
                return match after.chars().next() {
                    Some(token) => self.bad_include(arg, format!("%{token} is not supported")),
                    None => self.bad_include(arg, "a lone % at the end".to_owned()),
                };
            } else if let Some(after) = rest.strip_prefix("${") {
                let Some((name, after)) = after.split_once('}') else {
                    return self.bad_include(arg, "unterminated ${".to_owned());
                };
                let Some(value) = (self.var)(name) else {
                    return self.bad_include(arg, format!("${{{name}}} is not set"));
                };
                pattern.push_str(&value.to_string_lossy());
                rest = after;
            } else {
                pattern.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
        if pattern.starts_with('/') {
            Some(pattern)
        } else {
            Some(format!("{home}/.ssh/{pattern}"))
        }
    }

    fn bad_include(&mut self, arg: &str, reason: String) -> Option<String> {
        let arg = arg.to_owned();
        self.out.warnings.push(Warning::BadInclude { arg, reason });
        None
    }

    fn host(&mut self, tokens: &[String], what: &str, env: &str) {
        let literals: Vec<Host> = tokens.iter().filter_map(|t| Host::literal(t)).collect();
        let Some(host) = literals
            .iter()
            .find(|literal| !self.seen.contains(literal.as_str()))
            .cloned()
        else {
            return;
        };
        let mut aliases: Vec<String> = Vec::new();
        for literal in &literals {
            self.seen.insert(literal.0.clone());
            if *literal != host && !aliases.iter().any(|alias| alias == literal.as_str()) {
                aliases.push(literal.0.clone());
            }
        }
        self.out.entries.push(Entry {
            env: env.to_owned(),
            host,
            what: what.to_owned(),
            aliases,
        });
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Line<'a> {
    Comment(&'a str),
    Include(Vec<String>),
    Host(Vec<String>),
    Other,
}

fn classify(line: &str) -> Line<'_> {
    let line = line.trim();
    if let Some(comment) = line.strip_prefix('#') {
        return Line::Comment(comment.trim_start_matches(' '));
    }
    let (keyword, rest) = line.split_at(
        line.find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(line.len()),
    );
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    if keyword.eq_ignore_ascii_case("host") {
        Line::Host(arguments(rest))
    } else if keyword.eq_ignore_ascii_case("include") {
        Line::Include(arguments(rest))
    } else {
        Line::Other
    }
}

fn arguments(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut token = String::new();
    let mut begun = false;
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                begun = true;
            }
            '#' if !quoted && !begun => break,
            c if c.is_whitespace() && !quoted => {
                if begun {
                    args.push(std::mem::take(&mut token));
                    begun = false;
                }
            }
            c => {
                token.push(c);
                begun = true;
            }
        }
    }
    if begun {
        args.push(token);
    }
    args
}

fn read(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_line(args: &[&str]) -> Line<'static> {
        Line::Host(args.iter().map(|a| a.to_string()).collect())
    }

    #[test]
    fn classify_accepts_ssh_syntax() {
        assert_eq!(classify("Host a b"), host_line(&["a", "b"]));
        assert_eq!(classify("host=a"), host_line(&["a"]));
        assert_eq!(classify("\tHOST\ta"), host_line(&["a"]));
        assert_eq!(classify("  Host a # note b"), host_line(&["a"]));
        assert_eq!(classify("Host a\r"), host_line(&["a"]));
        assert_eq!(
            classify("  #  indented note  "),
            Line::Comment("indented note")
        );
        assert_eq!(classify("##  x"), Line::Comment("#  x"));
        assert_eq!(classify("Match host x"), Line::Other);
        assert_eq!(classify(""), Line::Other);
    }

    #[test]
    fn literal_rejects_patterns_and_negation() {
        assert!(Host::literal("web-1").is_some());
        for token in ["", "*", "web-?", "*.corp", "!neg", "a b"] {
            assert!(Host::literal(token).is_none(), "{token:?}");
        }
    }

    #[test]
    fn classify_groups_quotes_and_keeps_empty_arguments() {
        assert_eq!(classify("Host \"a b\" c"), host_line(&["a b", "c"]));
        assert_eq!(classify("Host a \"\" b"), host_line(&["a", "", "b"]));
        assert_eq!(classify("Host a#b"), host_line(&["a#b"]));
        assert_eq!(classify("Host = a"), host_line(&["a"]));
        assert_eq!(classify("Host"), host_line(&[]));
        assert_eq!(
            classify("Include a/*.conf \"b c\""),
            Line::Include(vec!["a/*.conf".to_owned(), "b c".to_owned()])
        );
    }

    #[test]
    fn expand_resolves_relative_against_dot_ssh() {
        let home = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/home");
        let var = |_: &str| None;
        let mut walk = Walk::new(&home, &var);
        let names: Vec<_> = walk
            .expand("config.d/*.conf")
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "10-lab.conf",
                "15-new.conf",
                "20-old.conf",
                "30-nocomment.conf",
                "40-crlf.conf",
                "9-late.conf"
            ]
        );
        assert!(walk.expand("config.d/nothing-*.conf").is_empty());
        assert!(
            walk.expand("config.d").is_empty(),
            "directories are not files"
        );
        assert_eq!(
            walk.expand("config.d/.hidden.conf").len(),
            1,
            "a name that spells out the dot still matches"
        );
        assert!(walk.out.warnings.is_empty());
    }

    #[test]
    fn expand_substitutes_tokens_without_touching_the_environment() {
        let var = |name: &str| (name == "SSHPICK_X").then(|| OsString::from("/x"));
        let mut walk = Walk::new(Path::new("/h"), &var);
        let mut pattern = |arg: &str| walk.pattern(arg);
        assert_eq!(pattern("${SSHPICK_X}/a").as_deref(), Some("/x/a"));
        assert_eq!(pattern("%d/b").as_deref(), Some("/h/b"));
        assert_eq!(pattern("%%c").as_deref(), Some("/h/.ssh/%c"));
        assert_eq!(pattern("~/d").as_deref(), Some("/h/d"));
        assert_eq!(pattern("~").as_deref(), Some("/h"));
        assert_eq!(pattern("~other/e").as_deref(), Some("/h/.ssh/~other/e"));
        assert_eq!(pattern("/etc/f").as_deref(), Some("/etc/f"));
        assert_eq!(
            pattern("config.d/*.conf").as_deref(),
            Some("/h/.ssh/config.d/*.conf")
        );
        assert_eq!(pattern("${UNSET}/g"), None);
        assert_eq!(pattern("${OPEN"), None);
        let reasons: Vec<_> = walk
            .out
            .warnings
            .iter()
            .map(|warning| warning.to_string())
            .collect();
        assert_eq!(
            reasons,
            [
                "Include ${UNSET}/g: skipped, ${UNSET} is not set",
                "Include ${OPEN: skipped, unterminated ${",
            ]
        );
    }

    #[test]
    fn home_is_escaped_so_brackets_are_not_a_glob_class() {
        let var = |_: &str| None;
        let mut walk = Walk::new(Path::new("/h[1]"), &var);
        assert_eq!(walk.pattern("~/x").as_deref(), Some("/h[[]1[]]/x"));
    }

    #[test]
    fn a_percent_token_sshpick_cannot_expand_skips_the_include_with_a_warning() {
        let var = |_: &str| None;
        let mut walk = Walk::new(Path::new("/h"), &var);
        assert_eq!(walk.pattern("hosts/%h.conf"), None);
        assert_eq!(walk.pattern("%u/x"), None);
        assert_eq!(walk.pattern("tail%"), None);
        assert_eq!(walk.pattern("%%h").as_deref(), Some("/h/.ssh/%h"));
        let reasons: Vec<_> = walk
            .out
            .warnings
            .iter()
            .map(|warning| warning.to_string())
            .collect();
        assert_eq!(
            reasons,
            [
                "Include hosts/%h.conf: skipped, %h is not supported",
                "Include %u/x: skipped, %u is not supported",
                "Include tail%: skipped, a lone % at the end",
            ]
        );
    }

    #[test]
    fn host_line_adds_one_row_and_keeps_the_other_names_as_aliases() {
        let var = |_: &str| None;
        let mut walk = Walk::new(Path::new("/h"), &var);
        let tokens = |line: &str| match classify(line) {
            Line::Host(tokens) => tokens,
            other => panic!("not a Host line: {other:?}"),
        };
        walk.host(&tokens("Host *.corp a b a !c d?"), "what", "ENV");
        walk.host(&tokens("Host b c"), "", "ENV");
        walk.host(&tokens("Host a b c"), "", "ENV");
        let rows: Vec<_> = walk
            .out
            .entries
            .iter()
            .map(|e| (e.host.as_str(), e.aliases.as_slice()))
            .collect();
        assert_eq!(
            rows,
            [
                ("a", ["b".to_owned()].as_slice()),
                ("c", ["b".to_owned()].as_slice())
            ]
        );
    }

    #[test]
    fn names_lead_with_the_shown_host_and_keep_the_aliases_in_line_order() {
        let var = |_: &str| None;
        let mut walk = Walk::new(Path::new("/h"), &var);
        let tokens = |line: &str| match classify(line) {
            Line::Host(tokens) => tokens,
            other => panic!("not a Host line: {other:?}"),
        };
        walk.host(&tokens("Host web web.corp *.corp web2"), "", "");
        walk.host(&tokens("Host solo"), "", "");
        let names: Vec<Vec<&str>> = walk
            .out
            .entries
            .iter()
            .map(|e| e.names().collect())
            .collect();
        assert_eq!(names, [vec!["web", "web.corp", "web2"], vec!["solo"]]);
    }

    #[test]
    fn warnings_read_as_one_line_each() {
        let path = PathBuf::from("/h/.ssh/a.conf");
        let cycle = Warning::Cycle { path: path.clone() };
        assert_eq!(cycle.to_string(), "/h/.ssh/a.conf: skipped, include cycle");
        let deep = Warning::TooDeep { path: path.clone() };
        assert_eq!(
            deep.to_string(),
            "/h/.ssh/a.conf: skipped, Include nested deeper than 16 levels"
        );
        let source = io::Error::from(io::ErrorKind::PermissionDenied);
        let unreadable = Warning::Unreadable { path, source };
        assert_eq!(
            unreadable.to_string(),
            "/h/.ssh/a.conf: skipped, permission denied"
        );
    }

    #[test]
    fn a_load_error_names_the_file_and_keeps_the_io_error_as_its_source() {
        let source = io::Error::from(io::ErrorKind::NotFound);
        let error = LoadError {
            path: PathBuf::from("/h/.ssh/config"),
            source,
        };
        assert_eq!(
            error.to_string(),
            "cannot read /h/.ssh/config: entity not found"
        );
        let cause = std::error::Error::source(&error).map(ToString::to_string);
        assert_eq!(cause.as_deref(), Some("entity not found"));
    }

    #[test]
    fn read_strips_a_bom_and_survives_invalid_utf8() {
        let dir = std::env::temp_dir().join(format!("sshpick-read-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("f.conf");
        fs::write(&path, b"\xef\xbb\xbf# a\xff b\n").unwrap();
        assert_eq!(read(&path).unwrap(), "# a\u{fffd} b\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}
