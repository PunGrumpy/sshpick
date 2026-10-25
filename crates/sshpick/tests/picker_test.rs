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

/// The hint row is the last row the help changes, so once it reads `esc close` every row above
/// it is on the screen.
fn help_is_drawn(frame: &pty::Frame) -> bool {
    frame.rows[23].contains("esc close")
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

#[test_case(80, "  ↑↓ move   enter ssh   esc quit   ? keys"; "every key")]
#[test_case(30, "  enter ssh   esc quit"; "no movement or help")]
#[test_case(20, "  enter esc"; "bare key names")]
fn the_hint_row_is_the_last_row_and_keeps_enter_and_esc(cols: u16, row: &str) {
    let mut options = Options::new("ui");
    options.cols = cols;
    let mut session = Session::start(options);
    let frame = session.wait_for("the hint row", |frame| !frame.rows[23].is_empty());
    assert_eq!(frame.rows[23], row, "{frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn the_route_row_says_where_the_selected_host_goes_and_the_hint_row_stays_last() {
    let mut session = started();
    let frame = session.wait_for("the route", |frame| frame.rows[22].contains("ubuntu@"));
    assert_eq!(
        frame.rows[22], "  → ubuntu@192.0.2.11:22 via stg-bastion",
        "{frame}"
    );
    assert_eq!(
        frame.rows[23], "  ↑↓ move   enter ssh   esc quit   ? keys",
        "{frame}"
    );
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn a_host_ssh_cannot_resolve_shows_the_first_line_of_the_failure_on_the_route_row() {
    let mut options = Options::new("inspect");
    options.cols = 120;
    options.typed = "broken";
    let mut session = Session::start(options);
    let frame = session.wait_for("the failure", |frame| frame.rows[22].contains("failed"));
    assert_eq!(
        frame.rows[22],
        "  → ssh -G failed: /home/me/.ssh/config.d/10-staging.conf line 6: Bad configuration option: bogus",
        "{frame}"
    );
    assert!(!frame.has("terminating"), "only the first line: {frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test_case("\x1bOP"; "f1")]
#[test_case("?"; "question mark on an empty filter")]
fn the_key_help_opens_in_place_and_does_not_take_the_keyboard(open: &str) {
    let mut session = started();
    assert!(!help_is_drawn(&session.frame()));
    session.send(open);
    let frame = session.wait_for("the key help", help_is_drawn);
    assert!(frame.has("toggle this help"), "{frame}");
    assert!(frame.has("> type to filter"), "{frame}");
    assert!(
        frame.has("▌ LAB         lab-a"),
        "the list is still there: {frame}"
    );
    assert!(frame.rows[13].starts_with("  Enter"), "{frame}");
    assert!(
        !frame.has("Keybinds") && !frame.has("┘"),
        "not a popup: {frame}"
    );
    assert_eq!(
        frame.rows[23], "  ↑↓ move   enter ssh   esc close",
        "{frame}"
    );

    session.send("consul");
    let frame = session.wait_for("the filter to apply with the help open", |frame| {
        help_is_drawn(frame)
            && frame.rows[0].starts_with("> consul")
            && frame.rows[0].ends_with("2/5")
            && frame.rows[3] == "  NEW STAGING newstg-consul2"
    });
    assert!(frame.has("toggle this help"), "{frame}");

    session.send("\x1b");
    let frame = session.wait_for("the key help to close, not the picker", |frame| {
        frame.alternate && frame.rows[23].contains("esc quit")
    });
    assert!(!frame.has("toggle this help"), "{frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn a_short_terminal_points_at_pick_help_for_the_keys_it_cannot_show() {
    let mut options = Options::new("ui");
    options.rows = 12;
    let mut session = Session::start(options);
    session.wait_for("the first frame", |frame| {
        frame.rows[11].contains("esc quit")
    });
    session.send("\x1bOP");
    let frame = session.wait_for("the key help", |frame| frame.rows[11].contains("esc close"));
    assert_eq!(frame.rows[10], "  +7 more · sshpick pick --help", "{frame}");
    assert!(frame.rows[7].starts_with("  Enter"), "{frame}");
    session.send("\x1b");
    session.wait_for("the key help to close", |frame| {
        frame.rows[11].contains("esc quit")
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
