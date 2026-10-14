use std::ffi::OsString;
use std::fmt::Display;
use std::io::{self, IsTerminal, StdoutLock, Write};

use crate::color::ColorConfig;

/// Everything sshpick says outside the picker. One value is built per run and passed down, so
/// no module reaches for a global.
#[derive(Debug)]
pub struct Output {
    stdout: ColorConfig,
    stderr: ColorConfig,
    human: bool,
    columns: usize,
}

const DEFAULT_COLUMNS: usize = 80;

impl Output {
    pub fn new(no_color: bool) -> Output {
        let var = |name: &str| std::env::var_os(name);
        let human = io::stdout().is_terminal();
        Output {
            stdout: ColorConfig::new(human, no_color, &var),
            stderr: ColorConfig::new(io::stderr().is_terminal(), no_color, &var),
            human,
            columns: columns(human, &var),
        }
    }

    /// True when stdout is a terminal: chrome such as `>` lines is for people only.
    pub fn human(&self) -> bool {
        self.human
    }

    /// The terminal's width when stdout is a terminal, else `$COLUMNS`, else 80.
    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn stdout_colors(&self) -> ColorConfig {
        self.stdout
    }

    pub fn stderr_colors(&self) -> ColorConfig {
        self.stderr
    }

    /// Where results go. A reader that hangs up (`sshpick --help | head -1`) is not an error:
    /// writes after the pipe closes are dropped and report success.
    pub fn result(&self) -> Stdout {
        Stdout {
            lock: io::stdout().lock(),
            closed: false,
        }
    }

    /// A `>` line on stderr, for people only: nothing is printed when stdout is not a terminal.
    pub fn log(&self, text: &str) {
        if self.human {
            eprintln!("{} {text}", self.stderr.gray(">"));
        }
    }

    pub fn error(&self, error: &dyn Display) {
        eprintln!("sshpick: {error}");
    }

    pub fn warn(&self, warning: &dyn Display) {
        eprintln!("sshpick: warning: {warning}");
    }
}

fn columns(human: bool, var: &dyn Fn(&str) -> Option<OsString>) -> usize {
    let from_terminal = || {
        let (columns, _) = crossterm::terminal::size().ok()?;
        Some(usize::from(columns)).filter(|columns| *columns > 0)
    };
    let from_env = || var("COLUMNS")?.to_str()?.trim().parse::<usize>().ok();
    human
        .then(from_terminal)
        .flatten()
        .or_else(from_env)
        .filter(|columns| *columns > 0)
        .unwrap_or(DEFAULT_COLUMNS)
}

pub struct Stdout {
    lock: StdoutLock<'static>,
    closed: bool,
}

impl Stdout {
    fn forgive<T: Default>(&mut self, result: io::Result<T>) -> io::Result<T> {
        match result {
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                self.closed = true;
                Ok(T::default())
            }
            other => other,
        }
    }
}

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Ok(buf.len());
        }
        let written = self.lock.write(buf);
        let written = self.forgive(written)?;
        Ok(if self.closed { buf.len() } else { written })
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        let flushed = self.lock.flush();
        self.forgive(flushed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            set.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[test]
    fn the_width_is_the_environments_unless_stdout_is_a_terminal() {
        assert_eq!(columns(false, &vars(&[])), 80);
        assert_eq!(columns(false, &vars(&[("COLUMNS", "132")])), 132);
        assert_eq!(columns(false, &vars(&[("COLUMNS", " 60 ")])), 60);
        assert_eq!(columns(false, &vars(&[("COLUMNS", "wide")])), 80);
        assert_eq!(columns(false, &vars(&[("COLUMNS", "0")])), 80);
    }

    #[test]
    fn a_broken_pipe_is_dropped_not_reported() {
        let mut out = Output::new(false).result();
        let broken: io::Result<()> = Err(io::Error::from(io::ErrorKind::BrokenPipe));
        out.forgive(broken).unwrap();
        assert!(out.closed);
        assert_eq!(
            out.write(b"after").unwrap(),
            5,
            "later writes are swallowed"
        );
        out.flush().unwrap();
    }

    #[test]
    fn any_other_error_is_reported() {
        let mut out = Output::new(false).result();
        let full: io::Result<()> = Err(io::Error::from(io::ErrorKind::StorageFull));
        let error = out.forgive(full).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::StorageFull);
        assert!(!out.closed);
    }
}
