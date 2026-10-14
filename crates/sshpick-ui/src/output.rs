use std::fmt::Display;
use std::io::{self, IsTerminal};

use crate::color::ColorConfig;

/// Everything sshpick says outside the picker. One value is built per run and passed down, so
/// no module reaches for a global.
#[derive(Debug)]
pub struct Output {
    stdout: ColorConfig,
    stderr: ColorConfig,
}

impl Output {
    pub fn new(no_color: bool) -> Output {
        let var = |name: &str| std::env::var_os(name);
        Output {
            stdout: ColorConfig::new(io::stdout().is_terminal(), no_color, &var),
            stderr: ColorConfig::new(io::stderr().is_terminal(), no_color, &var),
        }
    }

    pub fn stdout_colors(&self) -> ColorConfig {
        self.stdout
    }

    pub fn stderr_colors(&self) -> ColorConfig {
        self.stderr
    }

    pub fn error(&self, error: &dyn Display) {
        eprintln!("sshpick: {error}");
    }

    pub fn warn(&self, warning: &dyn Display) {
        eprintln!("sshpick: warning: {warning}");
    }
}
