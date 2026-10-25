//! The real binary on a pty, read through a terminal emulator. The picker's terminal handling
//! (raw mode, the alternate screen, what is left behind) is reachable no other way.

#[path = "common/pty.rs"]
mod pty;

use nix::sys::signal::Signal;
use pty::{Exit, Options, Session};
use test_case::test_case;

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
fn config_warnings_print_before_the_picker_opens_and_are_still_there_after_it_closes() {
    let mut options = Options::new("warns");
    options.shell = SHELL;
    // The warning names the fixture's absolute path; a wide pty keeps it on one row wherever the
    // repository is checked out.
    options.cols = 240;
    let mut session = Session::start(options);
    let open = session.wait_for("the first frame", |frame| frame.has("type to filter"));
    assert!(open.alternate, "{open}");
    assert!(!open.has("warning"), "{open}");

    session.send("\x1bOP");
    let help = session.wait_for("the key help", help_is_drawn);
    assert!(
        !help.has("Warnings") && !help.has("include cycle"),
        "{help}"
    );

    session.send("\x1b");
    session.wait_for("the key help to close", |frame| !help_is_drawn(frame));
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert_eq!(frame.rows[..3], SHELL[..], "{frame}");
    assert!(
        frame.rows[3].starts_with("! ") && frame.rows[3].ends_with("skipped, include cycle"),
        "{frame}"
    );
    assert_eq!(frame.cursor, (4, 0), "{frame}");
    assert!(frame.rows[4..].iter().all(String::is_empty), "{frame}");
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

#[test_case(Signal::SIGTERM ; "term")]
#[test_case(Signal::SIGHUP ; "hup")]
#[test_case(Signal::SIGINT ; "int")]
fn a_killed_picker_restores_the_terminal_and_dies_of_the_signal(signal: Signal) {
    let mut session = started();
    session.signal(signal);
    assert_eq!(session.exit(), Exit::Signal(signal));
    assert_restored(&session);
}
