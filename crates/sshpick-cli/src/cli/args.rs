use std::ffi::OsString;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Help,
    Version,
    List(String),
    Pick(String),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown option '{0}'\nRun 'sshpick --help' for usage.")]
pub(crate) struct UsageError(pub(crate) String);

pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, UsageError> {
    let mut args = args
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned());
    let mut list = false;
    let mut words = Vec::new();
    for arg in args.by_ref() {
        match arg.as_str() {
            "--" => break,
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "-l" | "--list" => list = true,
            _ if arg.starts_with('-') && arg.len() > 1 => return Err(UsageError(arg)),
            _ => words.push(arg),
        }
    }
    words.extend(args);
    let query = words.join(" ");
    Ok(if list {
        Command::List(query)
    } else {
        Command::Pick(query)
    })
}
