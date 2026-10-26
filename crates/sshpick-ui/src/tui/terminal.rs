//! Every way out ends in `restore_to`, which is idempotent, so the first to run wins and the rest
//! write nothing: `Tty::leave` before `exec`, the panic hook before the message is printed, and
//! `signals.rs` before the default action.

use std::io::{self, StdoutLock, Write};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::execute;
use crossterm::terminal::{
    BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{DefaultTerminal, Frame, Terminal, TerminalOptions, Viewport};

use super::plan::{Plan, Space};
use super::signals;

/// End a frame cut short, reset the attributes, show the cursor and leave the alternate screen.
const RESTORE: &[u8] = b"\x1b[?2026l\x1b[0m\x1b[?25h\x1b[?1049l";

static OPEN: AtomicBool = AtomicBool::new(false);
static HOOKS: Once = Once::new();

pub(super) struct Tty {
    terminal: DefaultTerminal,
    area: Rect,
}

impl Space {
    /// `TIOCGWINSZ`, an ioctl: the terminal is never asked with an escape sequence.
    pub(super) fn detect() -> Space {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((0, 0));
        Space::reported(cols, rows)
    }
}

impl Tty {
    pub(super) fn enter(plan: Plan) -> io::Result<Tty> {
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
        match Tty::build(plan) {
            Ok(tty) => Ok(tty),
            Err(error) => {
                restore();
                Err(error)
            }
        }
    }

    fn build(plan: Plan) -> io::Result<Tty> {
        execute!(io::stdout(), EnterAlternateScreen)?;
        let area = area(plan);
        // Fixed, not Fullscreen: the size comes from the resize events the picker already reads,
        // and the 80x24 that stands in for a terminal reporting 0x0.
        let terminal = Terminal::with_options(
            CrosstermBackend::new(io::stdout()),
            TerminalOptions {
                viewport: Viewport::Fixed(area),
            },
        )?;
        Ok(Tty { terminal, area })
    }

    /// A new size clears the screen and draws every cell again.
    pub(super) fn fit(&mut self, plan: Plan) -> io::Result<()> {
        let area = area(plan);
        if self.area != area {
            self.terminal.resize(area)?;
            self.area = area;
        }
        Ok(())
    }

    pub(super) fn draw(&mut self, render: impl FnOnce(&mut Frame)) -> io::Result<()> {
        execute!(self.terminal.backend_mut(), BeginSynchronizedUpdate)?;
        let drawn = self.terminal.draw(render).map(|_| ());
        execute!(self.terminal.backend_mut(), EndSynchronizedUpdate)?;
        drawn
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

fn area(plan: Plan) -> Rect {
    Rect::new(0, 0, plan.cols, plan.height())
}

fn restore() {
    let _ = restore_to(&OPEN, &mut io::stdout().lock());
}

/// Keeps stdout locked after restoring, so the picker cannot draw over the restored terminal in
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
    let screen = out.write_all(RESTORE).and_then(|()| out.flush());
    raw.and(screen)
}

#[cfg(test)]
mod tests {
    use super::*;

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
