use std::io::{self, IsTerminal};

use sshpick_core::{Host, Outcome, Picker, Previews};
use sshpick_ui::Screen;

use super::CommandBase;
use crate::cli;
use crate::cli::Exit;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("needs an interactive terminal; use --list to print the hosts")]
    NoTerminal,
    #[error("cannot start previews: {0}")]
    Previews(#[source] io::Error),
    #[error(transparent)]
    Terminal(io::Error),
    #[error("cannot run ssh: {0}")]
    Exec(#[source] io::Error),
}

pub(crate) fn run(base: &CommandBase, query: &str) -> Result<u8, cli::Error> {
    let loaded = base.load()?;
    let screen = Screen::new(&loaded.entries, &loaded.warnings);
    let mut picker = Picker::new(loaded.entries, query);
    if let Some(host) = picker.only_match() {
        base.warn_all(&loaded.warnings);
        return exec(host);
    }
    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        base.warn_all(&loaded.warnings);
        return Err(Error::NoTerminal.into());
    }
    let mut previews = match Previews::spawn() {
        Ok(previews) => previews,
        Err(error) => {
            base.warn_all(&loaded.warnings);
            return Err(Error::Previews(error).into());
        }
    };
    if let Some(host) = picker.selected() {
        previews.prefetch(host);
    }

    match sshpick_ui::run(&screen, &mut picker, &mut previews) {
        Ok(Outcome::Pick(host)) => exec(&host),
        Ok(Outcome::Cancel) => Ok(Exit::Cancelled.code()),
        Ok(Outcome::NoMatch) => Ok(Exit::NoMatch.code()),
        Err(error) => {
            base.warn_all(&loaded.warnings);
            Err(Error::Terminal(error).into())
        }
    }
}

fn exec(host: &Host) -> Result<u8, cli::Error> {
    Err(Error::Exec(sshpick_core::exec(host)).into())
}
