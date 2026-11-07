use std::io;

use serde::Serialize;
use sshpick_core::Warning;

use super::args::UsageError;
use super::spec;
use super::suggest;
use crate::commands::{WarningView, doctor, inspect, list, pick};

const CONFIG_DOCS: &str =
    "https://github.com/PunGrumpy/sshpick/blob/main/docs/configuration.md#config-conventions";

#[derive(Clone, Copy, Debug)]
pub(crate) enum Exit {
    Success = 0,
    NoMatch = 1,
    Error = 2,
    ExecFailed = 127,
    Cancelled = 130,
}

impl Exit {
    pub(crate) fn code(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Usage(#[from] UsageError),
    #[error(transparent)]
    Load(#[from] sshpick_core::LoadError),
    #[error(transparent)]
    Pick(#[from] pick::Error),
    #[error(transparent)]
    List(#[from] list::Error),
    #[error(transparent)]
    Inspect(#[from] inspect::Error),
    #[error(transparent)]
    Doctor(#[from] doctor::Error),
    #[error("no host matches \"{query}\"")]
    NoMatch {
        query: String,
        did_you_mean: Option<String>,
        warnings: Vec<Warning>,
    },
    #[error("cannot write to stdout: {0}")]
    Stdout(#[source] io::Error),
    #[error("cannot find the home directory")]
    NoHome,
    #[error("no hosts found in ~/.ssh/config or the files it Includes")]
    NoHosts { warnings: Vec<Warning> },
}

/// Why a command failed, for scripts. Add freely, never rename.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Reason {
    Usage,
    NoMatch,
    SeveralMatches,
    NoHosts,
    ConfigError,
    SshFailed,
    WriteFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    ActionRequired,
    Error,
}

impl Reason {
    /// Every reason, in the order the docs table lists them.
    #[cfg(test)]
    pub(crate) const ALL: [Reason; 7] = [
        Reason::Usage,
        Reason::NoMatch,
        Reason::SeveralMatches,
        Reason::NoHosts,
        Reason::ConfigError,
        Reason::SshFailed,
        Reason::WriteFailed,
    ];

    pub(crate) fn status(self) -> Status {
        match self {
            Reason::Usage | Reason::NoMatch | Reason::SeveralMatches => Status::ActionRequired,
            Reason::NoHosts | Reason::ConfigError | Reason::SshFailed | Reason::WriteFailed => {
                Status::Error
            }
        }
    }
}

#[derive(Serialize)]
struct Document {
    status: Status,
    reason: Reason,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<WarningView>,
}

impl Error {
    pub(crate) fn exit_code(&self) -> u8 {
        match self {
            Error::NoMatch { .. } => Exit::NoMatch,
            Error::Pick(pick::Error::Exec(_)) => Exit::ExecFailed,
            Error::Usage(_)
            | Error::Load(_)
            | Error::Pick(_)
            | Error::List(_)
            | Error::Inspect(_)
            | Error::Doctor(_)
            | Error::Stdout(_)
            | Error::NoHome
            | Error::NoHosts { .. } => Exit::Error,
        }
        .code()
    }

    pub(crate) fn reason(&self) -> Reason {
        match self {
            Error::Usage(_) => Reason::Usage,
            Error::NoMatch { .. } => Reason::NoMatch,
            Error::Pick(pick::Error::Several { .. })
            | Error::Inspect(inspect::Error::Several { .. }) => Reason::SeveralMatches,
            Error::NoHosts { .. } => Reason::NoHosts,
            Error::Load(_) | Error::NoHome => Reason::ConfigError,
            Error::Inspect(inspect::Error::Resolve(_))
            | Error::Pick(pick::Error::Exec(_) | pick::Error::Previews(_)) => Reason::SshFailed,
            Error::Stdout(_)
            | Error::List(list::Error::Write(_))
            | Error::Inspect(inspect::Error::Write(_))
            | Error::Doctor(doctor::Error::Write(_))
            | Error::Pick(pick::Error::Terminal(_)) => Reason::WriteFailed,
        }
    }

    fn warnings(&self) -> &[Warning] {
        match self {
            Error::NoMatch { warnings, .. }
            | Error::NoHosts { warnings }
            | Error::Inspect(inspect::Error::Several { warnings, .. }) => warnings,
            _ => &[],
        }
    }

    /// The failure as one line of JSON, for stderr under `--json`.
    pub(crate) fn document(&self) -> Result<String, serde_json::Error> {
        let reason = self.reason();
        serde_json::to_string(&Document {
            status: reason.status(),
            reason,
            message: self.to_string(),
            fix: self.fix(),
            warnings: self.warnings().iter().map(WarningView::of).collect(),
        })
    }

    pub(crate) fn fix(&self) -> Option<String> {
        match self {
            Error::Usage(UsageError::Option { option, command }) => {
                Some(accepted_options(option, *command))
            }
            Error::Usage(UsageError::Unexpected { command, word }) => {
                Some(accepted_options(word, Some(command)))
            }
            Error::Usage(UsageError::MissingQuery(command)) => Some(format!(
                "name a host, as in `sshpick {command} stg-consul1`, or list them with `sshpick ls`."
            )),
            Error::Usage(UsageError::Command(word)) => {
                Some(match suggest::closest(word, &spec::command_words()) {
                    Some(command) => format!(
                        "did you mean `sshpick help {command}`? See all commands with `sshpick --help`."
                    ),
                    None => "see all commands with `sshpick --help`.".to_owned(),
                })
            }
            Error::NoMatch {
                did_you_mean: Some(command),
                ..
            } => Some(format!("did you mean `sshpick {command}`?")),
            Error::NoMatch { .. } => Some("list every host with `sshpick ls`.".to_owned()),
            Error::Pick(error) => error.fix(),
            Error::Inspect(error) => error.fix(),
            Error::NoHosts { .. } => {
                Some("add `Host` lines; ENV and WHAT come from comments.".to_owned())
            }
            Error::Load(_)
            | Error::List(_)
            | Error::Doctor(_)
            | Error::Stdout(_)
            | Error::NoHome => None,
        }
    }

    pub(crate) fn docs(&self) -> Option<&'static str> {
        matches!(self, Error::NoHosts { .. }).then_some(CONFIG_DOCS)
    }
}

fn accepted_options(option: &str, command: Option<&'static str>) -> String {
    let options = spec::options_of(command);
    let mut visible: Vec<String> = options
        .iter()
        .filter(|option| !option.hidden)
        .map(|option| format!("--{}", option.long))
        .collect();
    visible.sort_unstable();
    let who = command.map_or_else(|| "sshpick".to_owned(), |name| format!("sshpick {name}"));
    let mut fix = format!("{who} accepts {}.", visible.join(", "));
    let spellings: Vec<(String, String)> = options
        .iter()
        .map(|option| (format!("--{}", option.long), format!("--{}", option.long)))
        .collect();
    let candidates: Vec<(&str, &str)> = spellings
        .iter()
        .map(|(word, answer)| (word.as_str(), answer.as_str()))
        .collect();
    if let Some(suggestion) = suggest::closest(option, &candidates) {
        fix.push_str(&format!(" Did you mean {suggestion}?"));
    }
    fix
}
