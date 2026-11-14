//! Every way out ends in `restore_to`, which is idempotent, so the first to run wins and the rest
//! write nothing: `Tty::leave` before `exec` or the printed host, the panic hook before the
//! message is printed, and `signals.rs` before the default action. The restore goes to the device
//! the picker drew on: stdout, or `/dev/tty` under `--print`.

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, IsTerminal, StdoutLock, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, Once, PoisonError};

use crossterm::execute;
use crossterm::terminal::{
    BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};
use sshpick_core::Surface;

use super::plan::{Plan, Space};
use super::signals;

/// End a frame cut short, reset the attributes, show the cursor and leave the alternate screen.
const RESTORE: &[u8] = b"\x1b[?2026l\x1b[0m\x1b[?25h\x1b[?1049l";

static OPEN: AtomicBool = AtomicBool::new(false);
static HOOKS: Once = Once::new();
/// `/dev/tty` while the picker draws on it. The restore takes it, so a frame drawn after the
/// restore fails instead of landing on the restored terminal.
static DEV_TTY: Mutex<Option<File>> = Mutex::new(None);

pub(super) struct Tty {
    terminal: Terminal<CrosstermBackend<Writer>>,
    area: Rect,
}

/// Where the picker's bytes go: stdout as it always did, or `/dev/tty` through [`DEV_TTY`].
enum Writer {
    Stdout(io::Stdout),
    Tty(BufWriter<DevTty>),
}

struct DevTty;

impl Writer {
    fn to(surface: Surface) -> Writer {
        match surface {
            Surface::Stdout => Writer::Stdout(io::stdout()),
            Surface::Tty => Writer::Tty(BufWriter::new(DevTty)),
        }
    }
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Writer::Stdout(out) => out.write(buf),
            Writer::Tty(out) => out.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Writer::Stdout(out) => out.flush(),
            Writer::Tty(out) => out.flush(),
        }
    }
}

impl Write for DevTty {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match dev_tty().as_mut() {
            Some(tty) => tty.write(buf),
            None => Err(io::Error::from(io::ErrorKind::BrokenPipe)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match dev_tty().as_mut() {
            Some(tty) => tty.flush(),
            None => Ok(()),
        }
    }
}

fn dev_tty() -> MutexGuard<'static, Option<File>> {
    DEV_TTY.lock().unwrap_or_else(PoisonError::into_inner)
}

fn open_dev_tty() -> io::Result<File> {
    OpenOptions::new().read(true).write(true).open("/dev/tty")
}

/// Whether the picker could draw on `surface`. Opens `/dev/tty` the way [`Tty::enter`] will, so
/// the rule's input and the device the picker opens are the same.
pub fn surface_is_terminal(surface: Surface) -> bool {
    match surface {
        Surface::Stdout => io::stdout().is_terminal(),
        Surface::Tty => open_dev_tty().is_ok_and(|tty| tty.is_terminal()),
    }
}

impl Space {
    /// `TIOCGWINSZ`, an ioctl: the terminal is never asked with an escape sequence.
    pub(super) fn detect() -> Space {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((0, 0));
        Space::reported(cols, rows)
    }
}

impl Tty {
    pub(super) fn enter(plan: Plan, surface: Surface) -> io::Result<Tty> {
        HOOKS.call_once(|| {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                restore();
                previous(info);
            }));
            // Picking still works without signal handling; a kill then leaves the terminal raw.
            let _ = signals::watch();
        });
        if surface == Surface::Tty {
            *dev_tty() = Some(open_dev_tty()?);
        }
        if let Err(error) = enable_raw_mode() {
            dev_tty().take();
            return Err(error);
        }
        OPEN.store(true, Ordering::SeqCst);
        match Tty::build(plan, surface) {
            Ok(tty) => Ok(tty),
            Err(error) => {
                restore();
                Err(error)
            }
        }
    }

    fn build(plan: Plan, surface: Surface) -> io::Result<Tty> {
        let mut writer = Writer::to(surface);
        execute!(writer, EnterAlternateScreen)?;
        let area = area(plan);
        // Fixed, not Fullscreen: the size comes from the resize events the picker already reads,
        // and the 80x24 that stands in for a terminal reporting 0x0.
        let terminal = Terminal::with_options(
            CrosstermBackend::new(writer),
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
        restore_on(&mut io::stdout().lock())
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
    let _ = restore_on(&mut io::stdout().lock());
}

/// Keeps stdout locked after restoring, so the picker cannot draw over the restored terminal in
/// the moment before the process ends. On `/dev/tty` the restore took the device instead.
pub(super) fn restore_and_hold() -> StdoutLock<'static> {
    let mut out = io::stdout().lock();
    let _ = restore_on(&mut out);
    out
}

/// Restores the device the picker opened: `/dev/tty` when it holds one, else stdout.
fn restore_on(stdout: &mut StdoutLock) -> io::Result<()> {
    match dev_tty().take() {
        Some(mut tty) => restore_to(&OPEN, &mut tty),
        None => restore_to(&OPEN, stdout),
    }
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
