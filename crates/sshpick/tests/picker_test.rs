//! The real binary on a pty, read through a terminal emulator. The picker's terminal handling
//! (raw mode, the alternate screen, what is left behind) is reachable no other way.

#[path = "common/pty.rs"]
mod pty;

use pty::{Exit, Options, Session};

const SHELL: &[&str] = &["$ git status", "clean", "$ sshpick"];

fn started() -> Session {
    let mut options = Options::new("ui");
    options.shell = SHELL;
    let session = Session::start(options);
    let frame = session.wait_for("the first frame", |frame| frame.has("type to filter"));
    assert!(frame.alternate, "{frame}");
    assert!(!session.cooked());
    session
}

/// The popup's bottom-right corner is the last cell the help paints.
fn help_is_drawn(frame: &pty::Frame) -> bool {
    frame.has("┘")
}

fn assert_restored(session: &Session) {
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert!(!frame.cursor_hidden, "{frame}");
    assert_eq!(frame.cursor, (3, 0), "{frame}");
    assert_eq!(frame.rows[..3], SHELL[..], "{frame}");
    assert!(frame.rows[3..].iter().all(String::is_empty), "{frame}");
    assert!(session.cooked());
}

#[test]
fn esc_exits_130_and_the_screen_is_restored() {
    let mut session = started();
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    assert_restored(&session);
}

#[test]
fn enter_runs_ssh_with_the_chosen_host_after_the_terminal_is_restored() {
    let mut session = started();
    session.send("lab-a");
    session.wait_for("one match", |frame| frame.has("1/5"));
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert_eq!(
        frame.rows[..5],
        [
            "$ git status",
            "clean",
            "$ sshpick",
            "COOKED",
            "ssh -- lab-a"
        ],
        "{frame}"
    );
    assert!(frame.rows[5..].iter().all(String::is_empty), "{frame}");
    assert!(session.cooked());
}

#[test]
fn f1_opens_the_key_help() {
    let mut session = started();
    assert!(!help_is_drawn(&session.frame()));
    session.send("\x1bOP");
    let frame = session.wait_for("the key help", help_is_drawn);
    assert!(frame.has("Keybinds"), "{frame}");
    assert!(frame.has("toggle this help"), "{frame}");
    assert!(frame.has("> type to filter"), "{frame}");
    session.send("\x1b");
    session.wait_for("the key help to close, not the picker", |frame| {
        frame.alternate && !frame.has("┘")
    });
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn typing_before_the_first_frame_is_not_lost() {
    let mut options = Options::new("ui");
    options.typed = "consul";
    let session = Session::start(options);
    session.wait_for("the typed query", |frame| {
        frame.alternate && frame.rows[0].starts_with("> consul") && frame.rows[0].ends_with("2/5")
    });
}
