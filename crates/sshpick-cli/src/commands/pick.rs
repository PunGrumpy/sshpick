use std::io;

use sshpick_core::{Host, Outcome, Picker, Previews, Recents, Refusal, Resolution};
use sshpick_ui::Screen;

use super::{CommandBase, did_you_mean, interaction, shell_words};
use crate::cli;
use crate::cli::Exit;

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
    let mut picker = Picker::new(loaded.entries, query, &Recents::none());
    let refusal = sshpick_core::refusal(&interaction(base.non_interactive));
    match (picker.resolve(), refusal) {
        (Resolution::One(entry), _) => return exec(base, &entry.host),
        (Resolution::Loose(entry), Some(Refusal::NonInteractive)) => {
            return Err(Error::Loose {
                query: query.to_owned(),
                nearest: entry.host.to_string(),
            }
            .into());
        }
        (Resolution::Loose(entry), _) => return exec(base, &entry.host),
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

    match sshpick_ui::run(&screen, &mut picker, &mut previews) {
        Ok(Outcome::Pick(host)) => exec(base, &host),
        Ok(Outcome::Cancel) => Ok(Exit::Cancelled.code()),
        Ok(Outcome::NoMatch) => Err(cli::Error::NoMatch {
            query: picker.query().to_owned(),
            did_you_mean: did_you_mean(picker.query()),
            warnings: Vec::new(),
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
