use std::ffi::OsString;
use std::io::Write;

use sshpick_ui::Output;

use self::args::{Args, Command};
use crate::commands::{CommandBase, inspect, list, pick};

mod args;
mod error;
mod help;
pub(crate) mod spec;
pub(crate) mod suggest;
#[cfg(test)]
mod test;

pub(crate) use self::error::{Error, Exit};

pub(crate) fn run(argv: impl IntoIterator<Item = OsString>) -> u8 {
    let args = args::parse(argv);
    let output = Output::new(args.global.no_color);
    match dispatch(args, &output) {
        Ok(code) => code,
        Err(error) => {
            output.error(&error, error.fix().as_deref(), error.docs());
            error.exit_code()
        }
    }
}

fn dispatch(args: Args, output: &Output) -> Result<u8, Error> {
    let non_interactive = args.global.non_interactive;
    match args.command? {
        Command::Help { topic } => {
            let spec = topic.and_then(spec::command);
            print(
                output,
                &help::render(spec, output.columns(), output.stdout_colors()),
            )
        }
        Command::Version => print(output, concat!("sshpick ", env!("CARGO_PKG_VERSION"), "\n")),
        Command::List {
            query,
            json,
            machine,
        } => list::run(
            &CommandBase::new(output, non_interactive)?,
            &list::Request {
                query: &query,
                json,
                machine,
                typed: args.typed.as_deref(),
            },
        ),
        Command::Inspect { query, json } => inspect::run(
            &CommandBase::new(output, non_interactive)?,
            &query,
            json,
            args.typed.as_deref().unwrap_or(spec::INSPECT.name),
        ),
        Command::Pick { query } => pick::run(&CommandBase::new(output, non_interactive)?, &query),
    }
}

fn print(output: &Output, text: &str) -> Result<u8, Error> {
    let mut out = output.result();
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(Error::Stdout)?;
    Ok(Exit::Success.code())
}
