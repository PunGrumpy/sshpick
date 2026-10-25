//! Every way out ends in `restore_to`, which is idempotent, so the first to run wins and the rest
//! write nothing: `Tty::leave` before `exec`, the panic hook before the message is printed, and
//! `signals.rs` before the default action. It erases the block from the extent the writer
//! published and leaves raw mode, so none of them needs the writer.

use std::io::{self, Stdout, StdoutLock, Write};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::layout::{Rect, Size};
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};

use super::inline::{Inline, restore_bytes};
use super::plan::{Plan, Space};
use super::signals;

static OPEN: AtomicBool = AtomicBool::new(false);
static EXTENT: AtomicU16 = AtomicU16::new(0);
static HOOKS: Once = Once::new();

pub(super) struct Tty {
    terminal: Terminal<Inline<'static, Stdout>>,
    shape: (Space, Plan),
}

impl Space {
    /// `TIOCGWINSZ`, an ioctl: the terminal is never asked with an escape sequence.
    pub(super) fn detect() -> Space {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((0, 0));
        Space::reported(cols, rows)
    }
}

impl Tty {
    pub(super) fn enter(space: Space, plan: Plan) -> io::Result<Tty> {
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
        match Tty::build(space, plan) {
            Ok(tty) => Ok(tty),
            Err(error) => {
                restore();
                Err(error)
            }
        }
    }

    fn build(space: Space, plan: Plan) -> io::Result<Tty> {
        let size = size(plan);
        let terminal = Terminal::with_options(
            Inline::open(io::stdout(), size, &EXTENT),
            // Fixed, not Inline: an Inline viewport asks the backend where the cursor is to place
            // itself, and this backend knows only the block's own rows.
            TerminalOptions {
                viewport: Viewport::Fixed(area(size)),
            },
        )?;
        Ok(Tty {
            terminal,
            shape: (space, plan),
        })
    }

    /// A resize that keeps the plan erases and redraws the block too, in case the terminal cut
    /// rows.
    pub(super) fn fit(&mut self, space: Space, plan: Plan) -> io::Result<()> {
        if self.shape == (space, plan) {
            return Ok(());
        }
        let size = size(plan);
        self.terminal.backend_mut().retarget(size);
        self.terminal.resize(area(size))?;
        self.shape = (space, plan);
        Ok(())
    }

    pub(super) fn draw(&mut self, render: impl FnOnce(&mut Frame)) -> io::Result<()> {
        self.terminal.draw(render).map(|_| ())
    }

    pub(super) fn leave(&mut self) -> io::Result<()> {
        restore_to(&OPEN, &EXTENT, &mut io::stdout().lock())
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

fn size(plan: Plan) -> Size {
    Size::new(plan.cols, plan.height())
}

fn area(size: Size) -> Rect {
    Rect::new(0, 0, size.width, size.height)
}

fn restore() {
    let _ = restore_to(&OPEN, &EXTENT, &mut io::stdout().lock());
}

/// Keeps stdout locked after restoring, so the picker cannot draw over the restored terminal in
/// the moment before the process ends.
pub(super) fn restore_and_hold() -> StdoutLock<'static> {
    let mut out = io::stdout().lock();
    let _ = restore_to(&OPEN, &EXTENT, &mut out);
    out
}

fn restore_to(open: &AtomicBool, extent: &AtomicU16, out: &mut impl Write) -> io::Result<()> {
    if !open.swap(false, Ordering::SeqCst) {
        return Ok(());
    }
    // Raw mode first: a terminal that no longer takes bytes must not keep raw mode on.
    let raw = disable_raw_mode();
    let block = out
        .write_all(&restore_bytes(extent.load(Ordering::SeqCst)))
        .and_then(|()| out.flush());
    raw.and(block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_restore_writes_once_however_often_it_runs() {
        let open = AtomicBool::new(true);
        let extent = AtomicU16::new(9);
        let mut out = Vec::new();
        restore_to(&open, &extent, &mut out).unwrap();
        restore_to(&open, &extent, &mut out).unwrap();
        assert_eq!(out, restore_bytes(9));
    }

    #[test]
    fn a_restore_before_the_terminal_is_taken_writes_nothing() {
        let open = AtomicBool::new(false);
        let mut out = Vec::new();
        restore_to(&open, &AtomicU16::new(9), &mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn a_restore_erases_the_extent_and_shows_the_cursor_without_leaving_any_screen() {
        assert_eq!(
            restore_bytes(3),
            b"\x1b[?2026l\x1b[0m\r\x1b[2K\n\x1b[2K\n\x1b[2K\x1b[2A\x1b[?25h"
        );
        assert_eq!(
            restore_bytes(0),
            b"\x1b[?2026l\x1b[0m\x1b[?25h",
            "before the first frame there is nothing to erase"
        );
    }
}
