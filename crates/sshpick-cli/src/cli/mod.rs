use std::ffi::OsString;
use std::io::Write;

use sshpick_ui::Output;

use self::args::Command;
use self::help::HELP;
use crate::commands::{CommandBase, list, pick};

mod args;
mod error;
mod help;
#[cfg(test)]
mod test;

pub(crate) use self::error::{Error, Exit};

pub(crate) fn run(argv: impl IntoIterator<Item = OsString>) -> u8 {
    let output = Output::new(false);
    match dispatch(argv, &output) {
        Ok(code) => code,
        Err(error) => {
            output.error(&error);
            error.exit_code()
        }
    }
}

fn dispatch(argv: impl IntoIterator<Item = OsString>, output: &Output) -> Result<u8, Error> {
    match args::parse(argv)? {
        Command::Help => print(output, HELP),
        Command::Version => print(output, concat!("sshpick ", env!("CARGO_PKG_VERSION"), "\n")),
        Command::List(query) => list::run(&CommandBase::new(output)?, &query),
        Command::Pick(query) => pick::run(&CommandBase::new(output)?, &query),
    }
}

fn print(output: &Output, text: &str) -> Result<u8, Error> {
    let mut out = output.result();
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(Error::Stdout)?;
    Ok(Exit::Success.code())
}
