//! In raw mode the keyboard's Ctrl-C is a key, not a signal, so this is about `kill`, a closed
//! window and a multiplexer going away. A thread, not a handler: the restore locks stdout, which a
//! signal handler may not.

use std::io;
use std::thread;

use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;
use signal_hook::low_level::emulate_default_handler;

use super::terminal::restore_and_hold;

pub(super) fn watch() -> io::Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP, SIGQUIT])?;
    thread::Builder::new()
        .name("signals".to_owned())
        .spawn(move || {
            // The loop never ends and `signals` is never dropped: dropping it unregisters the
            // handlers and leaves these signals ignored, so a SIGTERM between the picker closing
            // and `exec` would be swallowed.
            for signal in signals.forever() {
                let _stdout = restore_and_hold();
                let _ = emulate_default_handler(signal);
            }
        })?;
    Ok(())
}
