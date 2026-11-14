use std::io::{self, Write};
use std::time::SystemTime;

use sshpick_core::{Entry, Finish, History, Host, Outcome, Picker, Previews, Refusal, Resolution};
use sshpick_ui::Screen;

use super::{CommandBase, HostView, did_you_mean, interaction, shell_words, write_json};
use crate::cli;
use crate::cli::Exit;

/// How a pick ends, from argv: `--print` and `--json`. `--json` alone is a usage error, so
/// there is no JSON without printing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Landing {
    /// Exec `ssh -- <host>`.
    Connect,
    /// `host` and a newline on stdout.
    Print,
    /// The host's `{host, env, what, names}`, one element of `ls --json`'s `hosts`.
    PrintJson,
}

impl Landing {
    fn finish(self) -> Finish {
        match self {
            Landing::Connect => Finish::Connect,
            Landing::Print | Landing::PrintJson => Finish::Print,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("{}", several(.count, .query, .refusal))]
    Several {
        count: usize,
        query: String,
        names: Vec<String>,
        refusal: Refusal,
    },
    #[error("no host is named \"{query}\"; \"{nearest}\" matches loosely")]
    Loose { query: String, nearest: String },
    #[error("cannot start previews: {0}")]
    Previews(#[source] io::Error),
    #[error(transparent)]
    Terminal(io::Error),
    #[error("cannot run ssh: {0}")]
    Exec(#[source] io::Error),
    #[error("cannot print the host: {0}")]
    Write(#[source] io::Error),
}

impl Error {
    pub(crate) fn fix(&self) -> Option<String> {
        match self {
            Error::Several { query, .. } => {
                let list = if query.is_empty() {
                    "sshpick ls".to_owned()
                } else {
                    format!("sshpick ls {}", shell_words(query))
                };
                Some(format!(
                    "narrow the query to one host, or list them with `{list}`."
                ))
            }
            Error::Loose { query, nearest } => Some(format!(
                "connect with `sshpick --non-interactive {}`, or list candidates with `sshpick ls {}`.",
                shell_words(nearest),
                shell_words(query)
            )),
            Error::Exec(error) if error.kind() == io::ErrorKind::NotFound => {
                Some("install OpenSSH, or put `ssh` on your PATH.".to_owned())
            }
            Error::Previews(_) | Error::Terminal(_) | Error::Exec(_) | Error::Write(_) => None,
        }
    }
}

fn several(count: &usize, query: &str, refusal: &Refusal) -> String {
    let who = if query.is_empty() {
        format!("{count} hosts to choose from")
    } else {
        format!("{count} hosts match \"{query}\"")
    };
    format!("{who}, and {}", refusal.why())
}

/// Under `--print --json` the warnings are dropped: the document is one host, and stderr stays
/// empty. `sshpick ls --json` reports them.
pub(crate) fn run(base: &CommandBase, query: &str, landing: Landing) -> Result<u8, cli::Error> {
    let loaded = base.load()?;
    base.warn_all(&loaded.warnings);
    let surface = landing.finish().surface();
    let interaction = interaction(base.non_interactive, surface);
    let history = History::open(&base.home, &interaction);
    let screen = Screen::new(&loaded.entries);
    let recents = history.recents(SystemTime::now());
    let mut picker = Picker::new(loaded.entries, query, &recents);
    let refusal = sshpick_core::refusal(&interaction);
    match (picker.resolve(), refusal) {
        (Resolution::One(entry), _) => return land(base, &history, entry, landing),
        (Resolution::Loose(entry), Some(Refusal::NonInteractive)) => {
            return Err(Error::Loose {
                query: query.to_owned(),
                nearest: entry.host.to_string(),
            }
            .into());
        }
        (Resolution::Loose(entry), _) => return land(base, &history, entry, landing),
        (Resolution::NoMatch, Some(_)) => {
            return Err(cli::Error::NoMatch {
                query: query.to_owned(),
                did_you_mean: did_you_mean(query),
                warnings: Vec::new(),
            });
        }
        (Resolution::Many(count), Some(refusal)) => {
            return Err(Error::Several {
                count,
                query: query.to_owned(),
                names: picker
                    .listing()
                    .iter()
                    .map(|entry| entry.host.to_string())
                    .collect(),
                refusal,
            }
            .into());
        }
        (Resolution::NoMatch | Resolution::Many(_), None) => {}
    }
    let mut previews = Previews::spawn().map_err(Error::Previews)?;
    if let Some(host) = picker.selected() {
        previews.prefetch(host);
    }

    match sshpick_ui::run(&screen, &mut picker, &mut previews, surface) {
        Ok(Outcome::Pick(entry)) => land(base, &history, &entry, landing),
        Ok(Outcome::Cancel) => Ok(Exit::Cancelled.code()),
        Ok(Outcome::NoMatch) => Err(cli::Error::NoMatch {
            query: picker.query().to_owned(),
            did_you_mean: did_you_mean(picker.query()),
            warnings: Vec::new(),
        }),
        Err(error) => Err(Error::Terminal(error).into()),
    }
}

/// The one place a pick ends: ssh, or the host on stdout and exit 0. Either way a person chose
/// it, so it goes in the history, which is closed unless the picker could open.
fn land(
    base: &CommandBase,
    history: &History,
    entry: &Entry,
    landing: Landing,
) -> Result<u8, cli::Error> {
    if let Err(error) = history.record(&entry.host, SystemTime::now()) {
        base.output.warn(&error);
    }
    let printed = match landing {
        Landing::Connect => return exec(base, &entry.host),
        Landing::Print => {
            let mut out = base.output.result();
            writeln!(out, "{}", entry.host).and_then(|()| out.flush())
        }
        Landing::PrintJson => write_json(base.output.result(), &HostView::of(entry)),
    };
    printed.map_err(Error::Write)?;
    Ok(Exit::Success.code())
}

/// A fuzzy query or a single match hides which host it chose, so the command is said first.
fn exec(base: &CommandBase, host: &Host) -> Result<u8, cli::Error> {
    let command = sshpick_core::command(host).join(" ");
    base.output.log(&sshpick_core::printable(&command));
    Err(Error::Exec(sshpick_core::exec(host)).into())
}
