//! sshpick's command line: parses argv, runs the command, prints what went wrong and picks the
//! exit code. The `sshpick` binary is a single call into [`main`].
//!
//! `cli/` turns argv into a command and renders help and errors; `commands/` holds one file per
//! command with its own error type. `sshpick-ui` draws everything the user sees and
//! `sshpick-core` holds the rules.

mod cli;
mod commands;

/// Run sshpick with the process's arguments and return its exit status.
pub fn main() -> u8 {
    cli::run(std::env::args_os().skip(1))
}
