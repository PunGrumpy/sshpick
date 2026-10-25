//! Every way out ends in `restore_to`, which is idempotent, so the first to run wins and the rest
//! write nothing: `Tty::leave` before `exec`, the panic hook before the message is printed, and
//! `signals.rs` before the default action.

use std::io::{self, StdoutLock, Write};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::cursor::Show;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::{DefaultTerminal, Terminal};

use super::signals;

static OPEN: AtomicBool = AtomicBool::new(false);
static HOOKS: Once = Once::new();

pub(super) struct Tty {
    pub(super) terminal: DefaultTerminal,
}

impl Tty {
    pub(super) fn enter() -> io::Result<Tty> {
        HOOKS.call_once(|| {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                restore();
                previous(info);
            }));
            // Picking still works without signal handling; a kill then leaves the terminal raw.
            let _ = signals::watch();
        });
        enable_raw_mode()?;
        OPEN.store(true, Ordering::SeqCst);
        match Tty::build() {
            Ok(tty) => Ok(tty),
            Err(error) => {
                restore();
                Err(error)
            }
        }
    }

    fn build() -> io::Result<Tty> {
        execute!(io::stdout(), EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        Ok(Tty { terminal })
    }

    pub(super) fn leave(&mut self) -> io::Result<()> {
        restore_to(&OPEN, &mut io::stdout().lock())
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

fn restore() {
    let _ = restore_to(&OPEN, &mut io::stdout().lock());
}

/// Keeps stdout locked after restoring, so the picker cannot paint over the restored screen in
/// the moment before the process ends.
pub(super) fn restore_and_hold() -> StdoutLock<'static> {
    let mut out = io::stdout().lock();
    let _ = restore_to(&OPEN, &mut out);
    out
}

fn restore_to(open: &AtomicBool, out: &mut impl Write) -> io::Result<()> {
    if !open.swap(false, Ordering::SeqCst) {
        return Ok(());
    }
    // Raw mode first: a terminal that no longer takes bytes must not keep raw mode on.
    let raw = disable_raw_mode();
    let screen = execute!(out, Show, LeaveAlternateScreen);
    raw.and(screen)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESTORE: &[u8] = b"\x1b[?25h\x1b[?1049l";

    #[test]
    fn a_restore_writes_once_however_often_it_runs() {
        let open = AtomicBool::new(true);
        let mut out = Vec::new();
        restore_to(&open, &mut out).unwrap();
        restore_to(&open, &mut out).unwrap();
        assert_eq!(out, RESTORE);
    }

    #[test]
    fn a_restore_before_the_terminal_is_taken_writes_nothing() {
        let open = AtomicBool::new(false);
        let mut out = Vec::new();
        restore_to(&open, &mut out).unwrap();
        assert!(out.is_empty());
    }
}
