use std::ffi::OsString;

use super::spec::{self, CommandSpec};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Args {
    pub(crate) global: GlobalArgs,
    pub(crate) command: Command,
    /// The command word as typed (`ls`, not `list`); `None` for `pick` by default and `--list`.
    pub(crate) typed: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct GlobalArgs {
    pub(crate) no_color: bool,
    pub(crate) non_interactive: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Pick {
        query: String,
    },
    List {
        query: String,
        json: bool,
        /// `-l/--list`: tab-separated rows whatever the terminal is, for scripts.
        machine: bool,
    },
    Inspect {
        query: String,
        json: bool,
    },
    /// Help for one command, or for the root when `topic` is `None`.
    Help {
        topic: Option<&'static str>,
    },
    Version,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum UsageError {
    #[error("unknown option \"{option}\"")]
    Option {
        option: String,
        /// The command the option followed, so the fix lists what that command accepts.
        command: Option<&'static str>,
    },
    #[error("unknown command \"{0}\"")]
    Command(String),
    #[error("sshpick {0} needs a query")]
    MissingQuery(&'static str),
}

#[derive(Default)]
struct Flags {
    help: bool,
    version: bool,
    list: bool,
    no_color: bool,
    non_interactive: bool,
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
    let mut scoped = Vec::new();
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
            Some("non-interactive") => flags.non_interactive = true,
            _ => match spec::command_option(&arg) {
                Some(option) => scoped.push((arg, option.long)),
                None => {
                    unknown.get_or_insert(arg);
                }
            },
        }
    }
    let tail: Vec<String> = argv.collect();

    let named = words.first().and_then(|word| spec::command(word));
    let typed = named.map(|_| words.remove(0));
    words.extend(tail);

    let accepts =
        |long: &str| named.is_some_and(|spec| spec.options.iter().any(|o| o.long == long));
    let mut json = false;
    for (arg, long) in scoped {
        if accepts(long) {
            json |= long == "json";
        } else {
            unknown.get_or_insert(arg);
        }
    }

    let global = GlobalArgs {
        no_color: flags.no_color,
        non_interactive: flags.non_interactive,
    };
    let command = if flags.help {
        Command::Help {
            topic: named.map(|spec| spec.name),
        }
    } else if let Some(option) = unknown {
        return Err(UsageError::Option {
            option,
            command: named.map(|spec| spec.name),
        });
    } else if flags.version && named.is_none() {
        Command::Version
    } else {
        route(named, words, flags.list, json)?
    };
    Ok(Args {
        global,
        command,
        typed,
    })
}

fn route(
    named: Option<&CommandSpec>,
    words: Vec<String>,
    list: bool,
    json: bool,
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
    if name == spec::INSPECT.name {
        if words.is_empty() {
            return Err(UsageError::MissingQuery(spec::INSPECT.name));
        }
        return Ok(Command::Inspect { query, json });
    }
    Ok(if name == spec::LIST.name || list {
        Command::List {
            query,
            json,
            machine: list,
        }
    } else {
        Command::Pick { query }
    })
}
