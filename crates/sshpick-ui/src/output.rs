use std::fmt::Display;
use std::io::{self, IsTerminal, StdoutLock, Write};

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

    /// Where results go. A reader that hangs up (`sshpick --help | head -1`) is not an error:
    /// writes after the pipe closes are dropped and report success.
    pub fn result(&self) -> Stdout {
        Stdout {
            lock: io::stdout().lock(),
            closed: false,
        }
    }

    pub fn error(&self, error: &dyn Display) {
        eprintln!("sshpick: {error}");
    }

    pub fn warn(&self, warning: &dyn Display) {
        eprintln!("sshpick: warning: {warning}");
    }
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
