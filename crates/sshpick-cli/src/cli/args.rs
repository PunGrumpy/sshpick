use std::ffi::OsString;

use super::spec::{self, CommandSpec};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Args {
    pub(crate) global: GlobalArgs,
    pub(crate) command: Command,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct GlobalArgs {
    pub(crate) no_color: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Pick {
        query: String,
    },
    List {
        query: String,
        /// `-l/--list`: tab-separated rows whatever the terminal is, for scripts.
        machine: bool,
    },
    /// Help for one command, or for the root when `topic` is `None`.
    Help {
        topic: Option<&'static str>,
    },
    Version,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum UsageError {
    #[error("unknown option '{0}'\nRun 'sshpick --help' for usage.")]
    Option(String),
    #[error("unknown command '{0}'\nRun 'sshpick --help' for usage.")]
    Command(String),
}

#[derive(Default)]
struct Flags {
    help: bool,
    version: bool,
    list: bool,
    no_color: bool,
}

/// True when `--no-color` appears before a bare `--`, so a usage error can honour it before the
/// strict parse has run.
pub(crate) fn wants_no_color(argv: &[OsString]) -> bool {
    argv.iter()
        .take_while(|arg| *arg != "--")
        .any(|arg| arg == "--no-color")
}

/// Options are accepted anywhere before a bare `--`. The first positional selects a command when
/// it names one; otherwise every positional is `pick`'s query. Everything after `--` is query
/// text. `--help` beats every other error, and `--version` counts only when no command is named.
pub(crate) fn parse(argv: impl IntoIterator<Item = OsString>) -> Result<Args, UsageError> {
    let mut argv = argv
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned());
    let mut flags = Flags::default();
    let mut words = Vec::new();
    let mut unknown = None;
    for arg in argv.by_ref() {
        if arg == "--" {
            break;
        }
        if !arg.starts_with('-') || arg.len() == 1 {
            words.push(arg);
            continue;
        }
        match spec::option(&arg).map(|option| option.long) {
            Some("help") => flags.help = true,
            Some("version") => flags.version = true,
            Some("list") => flags.list = true,
            Some("no-color") => flags.no_color = true,
            _ => {
                unknown.get_or_insert(arg);
            }
        }
    }
    let tail: Vec<String> = argv.collect();

    let named = words.first().and_then(|word| spec::command(word));
    if named.is_some() {
        words.remove(0);
    }
    words.extend(tail);

    let global = GlobalArgs {
        no_color: flags.no_color,
    };
    let command = if flags.help {
        Command::Help {
            topic: named.map(|spec| spec.name),
        }
    } else if let Some(option) = unknown {
        return Err(UsageError::Option(option));
    } else if flags.version && named.is_none() {
        Command::Version
    } else {
        route(named, words, flags.list)?
    };
    Ok(Args { global, command })
}

fn route(
    named: Option<&CommandSpec>,
    words: Vec<String>,
    list: bool,
) -> Result<Command, UsageError> {
    let name = named.map_or(spec::PICK.name, |spec| spec.name);
    if name == spec::HELP.name {
        return match words.first() {
            None => Ok(Command::Help { topic: None }),
            Some(word) => spec::command(word)
                .map(|spec| Command::Help {
                    topic: Some(spec.name),
                })
                .ok_or_else(|| UsageError::Command(word.clone())),
        };
    }
    let query = words.join(" ");
    Ok(if name == spec::LIST.name || list {
        Command::List {
            query,
            machine: list,
        }
    } else {
        Command::Pick { query }
    })
}
