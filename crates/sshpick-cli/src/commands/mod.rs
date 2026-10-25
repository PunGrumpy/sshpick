use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use serde::Serialize;
use sshpick_core::{Entry, Interaction, Loaded, Warning};
use sshpick_ui::Output;

use crate::cli::{Error, spec, suggest};

pub(crate) mod doctor;
pub(crate) mod inspect;
pub(crate) mod list;
pub(crate) mod pick;

pub(crate) struct CommandBase<'a> {
    home: PathBuf,
    output: &'a Output,
    non_interactive: bool,
}

impl<'a> CommandBase<'a> {
    pub(crate) fn new(output: &'a Output, non_interactive: bool) -> Result<CommandBase<'a>, Error> {
        let home = std::env::home_dir().ok_or(Error::NoHome)?;
        Ok(CommandBase {
            home,
            output,
            non_interactive,
        })
    }

    pub(crate) fn load(&self) -> Result<Loaded, Error> {
        let loaded = sshpick_core::load(&self.home)?;
        if loaded.entries.is_empty() {
            self.warn_all(&loaded.warnings);
            return Err(Error::NoHosts);
        }
        Ok(loaded)
    }

    pub(crate) fn warn_all(&self, warnings: &[Warning]) {
        for warning in warnings {
            self.output.warn(warning);
        }
    }
}

pub(crate) fn note_if_also_a_host(output: &Output, word: &str, entries: &[Entry]) {
    if let Some(note) = collision_note(word, entries) {
        output.log(&note);
    }
}

pub(crate) fn interaction(non_interactive: bool) -> Interaction {
    Interaction {
        non_interactive,
        stdin_is_terminal: io::stdin().is_terminal(),
        stdout_is_terminal: io::stdout().is_terminal(),
        dumb: std::env::var_os("TERM").is_some_and(|term| term == "dumb"),
    }
}

fn collision_note(word: &str, entries: &[Entry]) -> Option<String> {
    let is_a_host = entries
        .iter()
        .any(|entry| entry.names().any(|name| name == word));
    is_a_host
        .then(|| format!("NOTE: \"{word}\" is also a host. Connect with `sshpick pick {word}`."))
}

pub(crate) fn did_you_mean(query: &str) -> Option<String> {
    let (first, rest) = query.split_once(' ').unwrap_or((query, ""));
    let command = suggest::closest(first, &spec::command_words())?;
    let takes_arguments = spec::command(command).is_some_and(|spec| !spec.arguments.is_empty());
    let rest = if takes_arguments { rest } else { "" };
    Some(format!("{command} {rest}").trim_end().to_owned())
}

#[derive(Serialize)]
pub(crate) struct HostView<'a> {
    pub(crate) host: &'a str,
    pub(crate) env: &'a str,
    pub(crate) what: &'a str,
    pub(crate) names: Vec<&'a str>,
}

impl<'a> HostView<'a> {
    pub(crate) fn of(entry: &'a Entry) -> HostView<'a> {
        HostView {
            host: entry.host.as_str(),
            env: &entry.env,
            what: &entry.what,
            names: entry.names().collect(),
        }
    }
}

pub(crate) fn write_json(mut out: impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut out, value)?;
    out.write_all(b"\n")?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn entries() -> Vec<Entry> {
        let home = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/words");
        sshpick_core::load(&home).unwrap().entries
    }

    #[test]
    fn a_command_word_that_is_also_a_host_or_an_alias_gets_a_note() {
        let entries = entries();
        assert_eq!(
            collision_note("list", &entries).as_deref(),
            Some("NOTE: \"list\" is also a host. Connect with `sshpick pick list`.")
        );
        assert_eq!(
            collision_note("ls", &entries).as_deref(),
            Some("NOTE: \"ls\" is also a host. Connect with `sshpick pick ls`."),
            "ls is an alias of the second host"
        );
        assert_eq!(
            collision_note("doctor", &entries).as_deref(),
            Some("NOTE: \"doctor\" is also a host. Connect with `sshpick pick doctor`.")
        );
        assert_eq!(collision_note("inspect", &entries), None);
        assert_eq!(collision_note("li", &entries), None, "exact names only");
    }

    #[test]
    fn a_misspelt_first_word_suggests_the_command_with_the_rest_of_the_query() {
        assert_eq!(did_you_mean("lst new"), Some("list new".to_owned()));
        assert_eq!(did_you_mean("lst"), Some("list".to_owned()));
        assert_eq!(
            did_you_mean("inspekt stg-consul1"),
            Some("inspect stg-consul1".to_owned())
        );
        assert_eq!(did_you_mean("consul"), None);
        assert_eq!(did_you_mean("list new"), None, "already a command");
        assert_eq!(did_you_mean("docter"), Some("doctor".to_owned()));
        assert_eq!(
            did_you_mean("docter new"),
            Some("doctor".to_owned()),
            "doctor takes no query, so the rest is not carried over"
        );
        assert_eq!(did_you_mean(""), None);
    }

    #[test]
    fn json_is_pretty_with_a_trailing_newline() {
        let mut out = Vec::new();
        let view = HostView {
            host: "a",
            env: "",
            what: "x",
            names: vec!["a", "b"],
        };
        write_json(&mut out, &view).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "{\n  \"host\": \"a\",\n  \"env\": \"\",\n  \"what\": \"x\",\n  \"names\": [\n    \"a\",\n    \"b\"\n  ]\n}\n"
        );
    }
}
