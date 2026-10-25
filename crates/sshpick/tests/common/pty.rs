//! The harness never answers a terminal query, so a picker that asks the terminal something and
//! waits shows up as a missed deadline, not as a hang the harness papers over. Every wait is a
//! condition with a deadline, never a sleep.
//!
//! The child writes a frame in several chunks, so a screen can be caught half drawn. ratatui
//! emits cells row by row: a condition that holds only once the last row a change touches is
//! on the screen means the whole change is.

use std::fmt;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::sys::termios::LocalFlags;
use nix::sys::wait::{WaitStatus, waitpid};
use nix::unistd::Pid;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

const WITHIN: Duration = Duration::from_secs(10);

pub struct Options {
    pub home: &'static str,
    pub cols: u16,
    pub rows: u16,
    pub term: &'static str,
    pub args: &'static [&'static str],
    /// Lines already on the screen when sshpick starts: the context the picker must give back.
    pub shell: &'static [&'static str],
    /// Keys typed before sshpick took the terminal. They wait in the pty's input queue.
    pub typed: &'static str,
}

impl Options {
    pub fn new(home: &'static str) -> Options {
        Options {
            home,
            cols: 80,
            rows: 24,
            term: "xterm-256color",
            args: &[],
            shell: &[],
            typed: "",
        }
    }
}

/// A process killed by a signal has no exit code, and the shell reports it as 128 plus the
/// signal, so the two are told apart here.
#[derive(Debug, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    Signal(Signal),
}

pub struct Frame {
    pub rows: Vec<String>,
    /// Row and column of the cursor.
    pub cursor: (u16, u16),
    pub alternate: bool,
    pub cursor_hidden: bool,
}

impl Frame {
    fn of(parser: &vt100::Parser) -> Frame {
        let screen = parser.screen();
        let (_, cols) = screen.size();
        Frame {
            rows: screen
                .rows(0, cols)
                .map(|row| row.trim_end().to_owned())
                .collect(),
            cursor: screen.cursor_position(),
            alternate: screen.alternate_screen(),
            cursor_hidden: screen.hide_cursor(),
        }
    }

    pub fn has(&self, text: &str) -> bool {
        self.rows.iter().any(|row| row.contains(text))
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (number, row) in self.rows.iter().enumerate() {
            writeln!(out, "{number:2}|{row}")?;
        }
        write!(
            out,
            "cursor {:?}, alternate screen {}, cursor hidden {}",
            self.cursor, self.alternate, self.cursor_hidden
        )
    }
}

struct Shared {
    parser: vt100::Parser,
    /// The child closed its end of the pty: everything it wrote is on the grid.
    closed: bool,
}

type Link = Arc<(Mutex<Shared>, Condvar)>;

pub struct Session {
    link: Link,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    exited: Receiver<nix::Result<WaitStatus>>,
    pid: Pid,
    reaped: bool,
}

impl Session {
    pub fn start(options: Options) -> Session {
        let Options {
            home,
            cols,
            rows,
            term,
            args,
            shell,
            typed,
        } = options;
        let pair = ok(
            native_pty_system().openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            }),
            "open a pty",
        );
        let mut reader = ok(pair.master.try_clone_reader(), "read from the pty");
        let mut writer = ok(pair.master.take_writer(), "write to the pty");

        let mut parser = vt100::Parser::new(rows, cols, 0);
        for line in shell {
            parser.process(line.as_bytes());
            parser.process(b"\r\n");
        }
        let link: Link = Arc::new((
            Mutex::new(Shared {
                parser,
                closed: false,
            }),
            Condvar::new(),
        ));
        let feed = Arc::clone(&link);
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(count) => {
                        let (shared, changed) = &*feed;
                        lock(shared).parser.process(&chunk[..count]);
                        changed.notify_all();
                    }
                    Err(error) if error.kind() == ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let (shared, changed) = &*feed;
            lock(shared).closed = true;
            changed.notify_all();
        });

        ok(writer.write_all(typed.as_bytes()), "type ahead");
        ok(writer.flush(), "type ahead");

        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_sshpick"));
        command.args(args);
        command.env_clear();
        command.env("HOME", fixtures().join(home));
        command.env(
            "PATH",
            format!(
                "{}:{}",
                fixtures().join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
        command.env("TERM", term);
        let child = ok(pair.slave.spawn_command(command), "start sshpick");
        drop(pair.slave);

        let Some(pid) = child
            .process_id()
            .and_then(|id| i32::try_from(id).ok())
            .map(Pid::from_raw)
        else {
            panic!("the child has no process id");
        };
        // `waitpid` rather than `Child::wait`, whose status names a signal only by its
        // locale-dependent description.
        let (sender, exited) = mpsc::channel();
        thread::spawn(move || {
            let status = loop {
                match waitpid(pid, None) {
                    Err(Errno::EINTR) => {}
                    status => break status,
                }
            };
            let _ = sender.send(status);
        });

        Session {
            link,
            master: pair.master,
            writer,
            exited,
            pid,
            reaped: false,
        }
    }

    pub fn send(&mut self, keys: &str) {
        ok(self.writer.write_all(keys.as_bytes()), "send keys");
        ok(self.writer.flush(), "send keys");
    }

    /// Not through the pty: a Ctrl-C typed into a raw terminal is a key, not a signal.
    pub fn signal(&self, signal: Signal) {
        ok(kill(self.pid, signal), "signal sshpick");
    }

    pub fn frame(&self) -> Frame {
        let (shared, _) = &*self.link;
        Frame::of(&lock(shared).parser)
    }

    pub fn wait_for(&self, what: &str, ready: impl Fn(&Frame) -> bool) -> Frame {
        let deadline = Instant::now() + WITHIN;
        let (shared, changed) = &*self.link;
        let mut guard = lock(shared);
        loop {
            let frame = Frame::of(&guard.parser);
            if ready(&frame) {
                return frame;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "gave up after {WITHIN:?} waiting for {what}; the screen is:\n{frame}"
            );
            guard = changed
                .wait_timeout(guard, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    pub fn exit(&mut self) -> Exit {
        let status = match self.exited.recv_timeout(WITHIN) {
            Ok(status) => ok(status, "wait for sshpick"),
            Err(_) => panic!(
                "sshpick did not exit within {WITHIN:?}; the screen is:\n{}",
                self.frame()
            ),
        };
        self.reaped = true;
        let deadline = Instant::now() + WITHIN;
        let (shared, changed) = &*self.link;
        let mut guard = lock(shared);
        while !guard.closed {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "the pty never closed after sshpick exited");
            guard = changed
                .wait_timeout(guard, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        match status {
            WaitStatus::Exited(_, code) => Exit::Code(code),
            WaitStatus::Signaled(_, signal, _) => Exit::Signal(signal),
            other => panic!("sshpick stopped without exiting: {other:?}"),
        }
    }

    pub fn cooked(&self) -> bool {
        let Some(termios) = self.master.get_termios() else {
            panic!("cannot read the pty's termios");
        };
        termios
            .local_flags
            .contains(LocalFlags::ICANON | LocalFlags::ECHO)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = kill(self.pid, Signal::SIGKILL);
        }
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

fn ok<T, E: fmt::Debug>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("cannot {what}: {error:?}"),
    }
}
