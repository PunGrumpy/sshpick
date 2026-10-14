use std::ffi::OsString;
use std::io::Write;

use sshpick_ui::Output;

use self::args::Command;
use crate::commands::{CommandBase, list, pick};

mod args;
mod error;
mod help;
mod spec;
#[cfg(test)]
mod test;

pub(crate) use self::error::{Error, Exit};

pub(crate) fn run(argv: impl IntoIterator<Item = OsString>) -> u8 {
    let argv: Vec<OsString> = argv.into_iter().collect();
    let output = Output::new(args::wants_no_color(&argv));
    match dispatch(argv, &output) {
        Ok(code) => code,
        Err(error) => {
            output.error(&error);
            error.exit_code()
        }
    }
}

fn dispatch(argv: Vec<OsString>, output: &Output) -> Result<u8, Error> {
    match args::parse(argv)?.command {
        Command::Help { topic } => {
            let spec = topic.and_then(spec::command);
            print(
                output,
                &help::render(spec, output.columns(), output.stdout_colors()),
            )
        }
        Command::Version => print(output, concat!("sshpick ", env!("CARGO_PKG_VERSION"), "\n")),
        Command::List { query, machine } => list::run(&CommandBase::new(output)?, &query, machine),
        Command::Pick { query } => pick::run(&CommandBase::new(output)?, &query),
    }
}

fn print(output: &Output, text: &str) -> Result<u8, Error> {
    let mut out = output.result();
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(Error::Stdout)?;
    Ok(Exit::Success.code())
}
