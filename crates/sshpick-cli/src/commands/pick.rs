use std::io::{self, IsTerminal};

use sshpick_core::{Host, Interaction, Outcome, Picker, Previews, Refusal, Resolution};
use sshpick_ui::Screen;

use super::{CommandBase, did_you_mean};
use crate::cli;
use crate::cli::Exit;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
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

impl Error {
    pub(crate) fn fix(&self) -> Option<String> {
        match self {
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
    format!("{who}, and {}", refusal.why())
}

pub(crate) fn run(base: &CommandBase, query: &str) -> Result<u8, cli::Error> {
    let loaded = base.load()?;
    base.warn_all(&loaded.warnings);
    let screen = Screen::new(&loaded.entries);
    let mut picker = Picker::new(loaded.entries, query);
    let refusal = sshpick_core::refusal(&Interaction {
        non_interactive: base.non_interactive,
        stdin_is_terminal: io::stdin().is_terminal(),
        stdout_is_terminal: io::stdout().is_terminal(),
        dumb: std::env::var_os("TERM").is_some_and(|term| term == "dumb"),
    });
    match (picker.resolve(), refusal) {
        (Resolution::One(host), _) => return exec(base, host),
        (Resolution::NoMatch, Some(_)) => {
            return Err(cli::Error::NoMatch {
                query: query.to_owned(),
                did_you_mean: did_you_mean(query),
            });
        }
        (Resolution::Many(count), Some(refusal)) => {
            return Err(Error::Several {
                count,
                query: query.to_owned(),
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

    match sshpick_ui::run(&screen, &mut picker, &mut previews) {
        Ok(Outcome::Pick(host)) => exec(base, &host),
        Ok(Outcome::Cancel) => Ok(Exit::Cancelled.code()),
        Ok(Outcome::NoMatch) => Err(cli::Error::NoMatch {
            query: picker.query().to_owned(),
            did_you_mean: did_you_mean(picker.query()),
        }),
        Err(error) => Err(Error::Terminal(error).into()),
    }
}

/// A fuzzy query or a single match hides which host it chose, so the command is said first.
fn exec(base: &CommandBase, host: &Host) -> Result<u8, cli::Error> {
    let command = sshpick_core::command(host).join(" ");
    base.output.log(&sshpick_core::printable(&command));
    Err(Error::Exec(sshpick_core::exec(host)).into())
}
