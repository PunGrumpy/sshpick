use std::io;

use ratatui::DefaultTerminal;

pub(super) struct Tty {
    pub(super) terminal: DefaultTerminal,
    active: bool,
}

impl Tty {
    pub(super) fn enter() -> io::Result<Tty> {
        match ratatui::try_init() {
            Ok(terminal) => Ok(Tty {
                terminal,
                active: true,
            }),
            Err(error) => {
                // A half-done init may have enabled raw mode before failing.
                let _ = ratatui::try_restore();
                Err(error)
            }
        }
    }

    pub(super) fn leave(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        let cursor = self.terminal.show_cursor();
        let restored = ratatui::try_restore();
        self.active = false;
        cursor.and(restored)
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}
