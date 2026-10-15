use std::io::{self, IsTerminal};

use sshpick_core::{Host, Outcome, Picker, Previews, Resolution};
use sshpick_ui::Screen;

use super::CommandBase;
use crate::cli;
use crate::cli::{Exit, spec, suggest};

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("no host matches \"{query}\"")]
    NoMatch {
        query: String,
        /// The command the first word looks like, with the rest of the query after it.
        did_you_mean: Option<String>,
    },
    #[error("{}", several(.count, .query, .refusal))]
    Several {
        count: usize,
        query: String,
        refusal: Refusal,
    },
    #[error("cannot start previews: {0}")]
    Previews(#[source] io::Error),
    #[error(transparent)]
    Terminal(io::Error),
    #[error("cannot run ssh: {0}")]
    Exec(#[source] io::Error),
}

/// Why the picker was not opened to settle several matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    NoTerminal,
    NonInteractive,
}

impl Error {
    pub(crate) fn fix(&self) -> Option<String> {
        match self {
            Error::NoMatch {
                did_you_mean: Some(command),
                ..
            } => Some(format!("did you mean `sshpick {command}`?")),
            Error::NoMatch { .. } => Some("list every host with `sshpick ls`.".to_owned()),
            Error::Several { query, .. } => {
                let list = if query.is_empty() {
                    "sshpick ls".to_owned()
                } else {
                    format!("sshpick ls {query}")
                };
                Some(format!(
                    "narrow the query to one host, or list them with `{list}`."
                ))
            }
            Error::Exec(error) if error.kind() == io::ErrorKind::NotFound => {
                Some("install OpenSSH, or put `ssh` on your PATH.".to_owned())
            }
            Error::Previews(_) | Error::Terminal(_) | Error::Exec(_) => None,
        }
    }
}

fn several(count: &usize, query: &str, refusal: &Refusal) -> String {
    let who = if query.is_empty() {
        format!("{count} hosts to choose from")
    } else {
        format!("{count} hosts match \"{query}\"")
    };
    let why = match refusal {
        Refusal::NoTerminal => "the picker needs an interactive terminal",
        Refusal::NonInteractive => "--non-interactive never opens the picker",
    };
    format!("{who}, and {why}")
}

pub(crate) fn run(base: &CommandBase, query: &str) -> Result<u8, cli::Error> {
    let loaded = base.load()?;
    let screen = Screen::new(&loaded.entries, &loaded.warnings);
    let mut picker = Picker::new(loaded.entries, query);
    let refusal = if base.non_interactive {
        Some(Refusal::NonInteractive)
    } else if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        Some(Refusal::NoTerminal)
    } else {
        None
    };
    match (picker.resolve(), refusal) {
        (Resolution::One(host), _) => {
            base.warn_all(&loaded.warnings);
            return exec(host);
        }
        (Resolution::NoMatch, Some(_)) => {
            base.warn_all(&loaded.warnings);
            return Err(Error::NoMatch {
                query: query.to_owned(),
                did_you_mean: did_you_mean(query),
            }
            .into());
        }
        (Resolution::Many(count), Some(refusal)) => {
            base.warn_all(&loaded.warnings);
            return Err(Error::Several {
                count,
                query: query.to_owned(),
                refusal,
            }
            .into());
        }
        (Resolution::NoMatch | Resolution::Many(_), None) => {}
    }
    let mut previews = match Previews::spawn() {
        Ok(previews) => previews,
        Err(error) => {
            base.warn_all(&loaded.warnings);
            return Err(Error::Previews(error).into());
        }
    };
    if let Some(host) = picker.selected() {
        previews.prefetch(host);
    }

    match sshpick_ui::run(&screen, &mut picker, &mut previews) {
        Ok(Outcome::Pick(host)) => exec(&host),
        Ok(Outcome::Cancel) => Ok(Exit::Cancelled.code()),
        Ok(Outcome::NoMatch) => Ok(Exit::NoMatch.code()),
        Err(error) => {
            base.warn_all(&loaded.warnings);
            Err(Error::Terminal(error).into())
        }
    }
}

fn exec(host: &Host) -> Result<u8, cli::Error> {
    Err(Error::Exec(sshpick_core::exec(host)).into())
}

/// A first word one edit away from a command, as the whole command line that was probably meant.
fn did_you_mean(query: &str) -> Option<String> {
    let (first, rest) = query.split_once(' ').unwrap_or((query, ""));
    let command = suggest::closest(first, &spec::command_words())?;
    Some(format!("{command} {rest}").trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_misspelt_first_word_suggests_the_command_with_the_rest_of_the_query() {
        assert_eq!(did_you_mean("lst new"), Some("list new".to_owned()));
        assert_eq!(did_you_mean("lst"), Some("list".to_owned()));
        assert_eq!(did_you_mean("consul"), None);
        assert_eq!(did_you_mean("list new"), None, "already a command");
        assert_eq!(did_you_mean(""), None);
    }
}
