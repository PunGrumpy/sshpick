use super::args::UsageError;
use crate::commands::{list, pick};

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
    #[error("cannot find the home directory")]
    NoHome,
    #[error(
        "no hosts found in ~/.ssh/config or the files it Includes\n\
         Hosts come from 'Host' lines; see README for the ENV/WHAT comment conventions."
    )]
    NoHosts,
}

impl Error {
    pub(crate) fn exit_code(&self) -> u8 {
        match self {
            Error::Pick(pick::Error::Exec(_)) => Exit::ExecFailed,
            Error::Usage(_)
            | Error::Load(_)
            | Error::Pick(_)
            | Error::List(_)
            | Error::NoHome
            | Error::NoHosts => Exit::Error,
        }
        .code()
    }
}
