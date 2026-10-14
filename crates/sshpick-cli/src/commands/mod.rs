use std::path::PathBuf;

use sshpick_core::{Loaded, Warning};
use sshpick_ui::Output;

use crate::cli::Error;

pub(crate) mod list;
pub(crate) mod pick;

pub(crate) struct CommandBase<'a> {
    home: PathBuf,
    output: &'a Output,
}

impl<'a> CommandBase<'a> {
    pub(crate) fn new(output: &'a Output) -> Result<CommandBase<'a>, Error> {
        let home = std::env::home_dir().ok_or(Error::NoHome)?;
        Ok(CommandBase { home, output })
    }

    pub(crate) fn load(&self) -> Result<Loaded, Error> {
        let loaded = sshpick_core::load(&self.home)?;
        if loaded.entries.is_empty() {
            self.warn_all(&loaded.warnings);
            return Err(Error::NoHosts);
        }
        Ok(loaded)
    }

    pub(crate) fn warn_all(&self, warnings: &[Warning]) {
        for warning in warnings {
            self.output.warn(warning);
        }
    }
}
