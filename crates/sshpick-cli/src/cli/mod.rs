use std::ffi::OsString;

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
        Command::Help => {
            print!("{HELP}");
            Ok(Exit::Success.code())
        }
        Command::Version => {
            println!("sshpick {}", env!("CARGO_PKG_VERSION"));
            Ok(Exit::Success.code())
        }
        Command::List(query) => list::run(&CommandBase::new(output)?, &query),
        Command::Pick(query) => pick::run(&CommandBase::new(output)?, &query),
    }
}
