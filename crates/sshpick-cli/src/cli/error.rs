use std::io;

use super::args::UsageError;
use super::spec;
use super::suggest;
use crate::commands::{inspect, list, pick};

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
    #[error("no host matches \"{query}\"")]
    NoMatch {
        query: String,
        did_you_mean: Option<String>,
    },
    #[error("cannot write to stdout: {0}")]
    Stdout(#[source] io::Error),
    #[error("cannot find the home directory")]
    NoHome,
    #[error("no hosts found in ~/.ssh/config or the files it Includes")]
    NoHosts,
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
            | Error::Stdout(_)
            | Error::NoHome
            | Error::NoHosts => Exit::Error,
        }
        .code()
    }

    pub(crate) fn fix(&self) -> Option<String> {
        match self {
            Error::Usage(UsageError::Option { option, command }) => {
                Some(accepted_options(option, *command))
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
            Error::NoHosts => Some("add `Host` lines; ENV and WHAT come from comments.".to_owned()),
            Error::Load(_) | Error::List(_) | Error::Stdout(_) | Error::NoHome => None,
        }
    }

    pub(crate) fn docs(&self) -> Option<&'static str> {
        matches!(self, Error::NoHosts).then_some(CONFIG_DOCS)
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
