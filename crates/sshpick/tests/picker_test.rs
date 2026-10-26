//! The real binary on a pty, read through a terminal emulator. The picker's terminal handling
//! (raw mode, the alternate screen, what is left behind) is reachable no other way.

#[path = "common/pty.rs"]
mod pty;

use nix::sys::signal::Signal;
use pty::{Exit, Frame, Options, Session};
use test_case::test_case;

const SHELL: &[&str] = &["$ git status", "clean", "$ sshpick"];
/// The picker fills the default 24 rows: the prompt on the top one, the route and the hint row on
/// the last two.
const ROUTE: usize = 22;
const HINT: usize = 23;

fn started() -> Session {
    let mut options = Options::new("ui");
    options.shell = SHELL;
    let session = Session::start(options);
    session.wait_for("the first frame", drawn);
    session
}

/// The hint row is the last row a frame writes, and the cursor goes back to the prompt after it,
/// so once both hold the whole frame is on the screen.
fn drawn(frame: &Frame) -> bool {
    frame.alternate && frame.rows[HINT].contains("esc quit") && frame.cursor == (0, 2)
}

fn resolved(frame: &Frame) -> bool {
    frame.rows[ROUTE].contains("ubuntu@")
}

/// The shell's screen as it was before sshpick started, with nothing of the picker on it.
fn assert_restored(session: &Session) {
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert!(!frame.cursor_hidden, "{frame}");
    assert_eq!(frame.cursor, (3, 0), "{frame}");
    assert_eq!(frame.rows[..3], SHELL[..], "{frame}");
    assert!(frame.rows[3..].iter().all(String::is_empty), "{frame}");
    assert!(
        session.scrollback().is_empty(),
        "{:?}",
        session.scrollback()
    );
    assert!(session.cooked());
}

#[test]
fn the_picker_fills_the_screen_with_the_hint_row_at_the_bottom() {
    let session = started();
    let frame = session.frame();
    assert!(
        frame.rows[0].starts_with("> type to filter") && frame.rows[0].ends_with("5/5"),
        "{frame}"
    );
    assert_eq!(
        frame.rows[1], "  ENV         HOST           WHAT",
        "{frame}"
    );
    assert!(frame.rows[2].starts_with("▌ LAB         lab-a"), "{frame}");
    assert!(
        frame.rows[7..ROUTE].iter().all(String::is_empty),
        "the rows the hosts do not need are blank: {frame}"
    );
    assert_eq!(
        frame.rows[HINT], "  ↑↓ move   enter ssh   esc quit   ? keys",
        "{frame}"
    );
    assert!(
        !frame.has("$ git status"),
        "the shell is behind it: {frame}"
    );
    assert!(session.scrollback().is_empty());
    assert!(!session.cooked());
}

#[test]
fn esc_exits_130_and_gives_the_shell_its_screen_back() {
    let mut session = started();
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    assert_restored(&session);
}

#[test]
fn enter_says_which_host_runs_under_the_command_then_ssh_starts_on_a_cooked_terminal() {
    let mut session = started();
    session.send("lab-a");
    session.wait_for("one match", |frame| frame.rows[0].ends_with("1/5"));
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert_eq!(
        frame.rows[..6],
        [
            "$ git status",
            "clean",
            "$ sshpick",
            "> ssh -- lab-a",
            "COOKED",
            "ssh -- lab-a"
        ],
        "{frame}"
    );
    assert!(frame.rows[6..].iter().all(String::is_empty), "{frame}");
    assert!(session.cooked());
}

#[test]
fn a_query_with_one_match_says_which_host_runs_before_ssh_starts() {
    let mut options = Options::new("ui");
    options.shell = &["$ sshpick lab-a"];
    options.args = &["lab-a"];
    let mut session = Session::start(options);
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert_eq!(
        frame.rows[..4],
        [
            "$ sshpick lab-a",
            "> ssh -- lab-a",
            "COOKED",
            "ssh -- lab-a"
        ],
        "{frame}"
    );
    assert!(frame.rows[4..].iter().all(String::is_empty), "{frame}");
}

#[test]
fn enter_with_nothing_matched_says_so_and_exits_1() {
    let mut session = started();
    session.send("zzzz");
    session.wait_for("no match", |frame| {
        frame.rows[2] == "  no matching hosts" && frame.rows[HINT] == "  esc quit   F1 keys"
    });
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(1));
    let frame = session.frame();
    assert_eq!(
        frame.rows[..5],
        [
            "$ git status",
            "clean",
            "$ sshpick",
            "Error: no host matches \"zzzz\"",
            "Fix: list every host with `sshpick ls`."
        ],
        "{frame}"
    );
    assert!(frame.rows[5..].iter().all(String::is_empty), "{frame}");
    assert_eq!(frame.cursor, (5, 0), "{frame}");
    assert!(session.cooked());
}

#[test]
fn a_full_shell_screen_comes_back_as_it_was_and_the_scrollback_is_untouched() {
    const HISTORY: [&str; 23] = [
        "line 1",
        "line 2",
        "line 3",
        "line 4",
        "line 5",
        "line 6",
        "line 7",
        "line 8",
        "line 9",
        "line 10",
        "line 11",
        "line 12",
        "line 13",
        "line 14",
        "line 15",
        "line 16",
        "line 17",
        "line 18",
        "line 19",
        "line 20",
        "line 21",
        "line 22",
        "$ sshpick",
    ];
    let mut options = Options::new("ui");
    options.shell = &HISTORY;
    let mut session = Session::start(options);
    let frame = session.wait_for("the first frame", drawn);
    assert!(frame.rows[0].starts_with("> type to filter"), "{frame}");
    assert!(
        session.scrollback().is_empty(),
        "nothing scrolled to make room"
    );

    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    let frame = session.frame();
    assert_eq!(frame.rows[..23], HISTORY[..], "{frame}");
    assert!(frame.rows[23].is_empty(), "{frame}");
    assert_eq!(frame.cursor, (23, 0), "{frame}");
    assert!(
        session.scrollback().is_empty(),
        "{:?}",
        session.scrollback()
    );
}

#[test_case(80, "  ↑↓ move   enter ssh   esc quit   ? keys"; "every key")]
#[test_case(30, "  enter ssh   esc quit"; "no movement or help")]
#[test_case(20, "  enter esc"; "bare key names")]
fn the_hint_row_is_the_last_row_of_the_screen_and_keeps_enter_and_esc(cols: u16, row: &str) {
    let mut options = Options::new("ui");
    options.cols = cols;
    let mut session = Session::start(options);
    session.wait_for("the hint row", |frame| frame.rows[HINT] == row);
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn the_route_row_says_where_the_selected_host_goes_and_the_hint_row_stays_last() {
    let mut session = started();
    let frame = session.wait_for("the route", resolved);
    assert_eq!(
        frame.rows[ROUTE], "  → ubuntu@192.0.2.11:22 via stg-bastion",
        "{frame}"
    );
    assert_eq!(
        frame.rows[HINT], "  ↑↓ move   enter ssh   esc quit   ? keys",
        "{frame}"
    );
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn a_host_ssh_cannot_resolve_shows_the_first_line_of_the_failure_on_the_route_row() {
    let mut options = Options::new("inspect");
    // One column short of the panel, so the failure is on the route row.
    options.cols = 99;
    options.typed = "broken";
    let mut session = Session::start(options);
    let frame = session.wait_for("the failure", |frame| frame.rows[ROUTE].contains("failed"));
    assert_eq!(
        frame.rows[ROUTE],
        "  → ssh -G failed: /home/me/.ssh/config.d/10-staging.conf line 6: Bad configuration option: bogus",
        "{frame}"
    );
    assert!(!frame.has("terminating"), "only the first line: {frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn a_wide_terminal_shows_the_route_beside_the_list_and_enter_leaves_none_of_it() {
    let mut options = Options::new("ui");
    options.shell = SHELL;
    options.cols = 120;
    let mut session = Session::start(options);
    let frame = session.wait_for("the panel", |frame| {
        frame.rows[2].ends_with("│ user       ubuntu")
    });
    assert!(frame.rows[1].ends_with("│ lab-a"), "{frame}");
    assert!(
        frame.rows[3].ends_with("│ hostname   192.0.2.11"),
        "{frame}"
    );
    assert!(frame.rows[HINT].contains("esc quit"), "{frame}");
    assert!(!frame.has("→"), "the route is drawn once: {frame}");
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert_eq!(
        frame.rows[..6],
        [
            "$ git status",
            "clean",
            "$ sshpick",
            "> ssh -- lab-a",
            "COOKED",
            "ssh -- lab-a"
        ],
        "{frame}"
    );
    assert!(frame.rows[6..].iter().all(String::is_empty), "{frame}");
}

#[test_case("\x1bOP"; "f1")]
#[test_case("?"; "question mark on an empty filter")]
fn the_key_help_takes_the_blank_rows_and_esc_closes_it_before_the_picker(open: &str) {
    let mut session = started();
    session.send(open);
    let frame = session.wait_for("the key help", |frame| {
        frame.rows[HINT].contains("esc close")
    });
    assert!(frame.rows[0].starts_with("> type to filter"), "{frame}");
    assert!(
        frame.rows[2].starts_with("▌ LAB         lab-a"),
        "the list is still there: {frame}"
    );
    assert!(
        frame.rows[11].starts_with("  → "),
        "the route under the hosts: {frame}"
    );
    assert!(frame.rows[12].is_empty(), "{frame}");
    assert!(frame.rows[13].starts_with("  Enter"), "{frame}");
    assert!(frame.has("toggle this help"), "{frame}");
    assert_eq!(
        frame.rows[HINT], "  ↑↓ move   enter ssh   esc close",
        "{frame}"
    );
    assert!(
        !frame.has("Keybinds") && !frame.has("┘"),
        "not a popup: {frame}"
    );

    session.send("consul");
    let frame = session.wait_for("the filter to apply with the help open", |frame| {
        frame.rows[HINT].contains("esc close")
            && frame.rows[0].starts_with("> consul")
            && frame.rows[0].ends_with("2/5")
            && frame.rows[3] == "  NEW STAGING newstg-consul2"
    });
    assert!(frame.has("toggle this help"), "{frame}");

    session.send("\x1b");
    let frame = session.wait_for("the key help to close, not the picker", |frame| {
        frame.rows[HINT].contains("esc quit")
    });
    assert!(frame.alternate && !frame.has("toggle this help"), "{frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    assert_restored(&session);
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
fn a_resize_redraws_one_picker_on_the_whole_screen() {
    let mut session = started();
    session.wait_for("the route", resolved);
    for (cols, rows) in [(50, 24), (120, 24), (40, 24), (80, 12)] {
        session.resize(cols, rows);
        let hint = usize::from(rows) - 1;
        let frame = session.wait_for(&format!("a frame at {cols}x{rows}"), |frame| {
            frame.rows[0].chars().count() == usize::from(cols)
                && frame.rows[0].ends_with("5/5")
                && frame.rows[hint].contains("esc quit")
                && frame.cursor == (0, 2)
        });
        let prompts = frame
            .rows
            .iter()
            .filter(|row| row.starts_with("> "))
            .count();
        let hints = frame
            .rows
            .iter()
            .filter(|row| row.contains("esc quit"))
            .count();
        assert_eq!((prompts, hints), (1, 1), "{frame}");
        if cols >= 100 {
            assert!(frame.rows[2].ends_with("user       ubuntu"), "{frame}");
        } else {
            assert!(frame.rows[hint - 1].contains("ubuntu@"), "{frame}");
        }
    }
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    assert_restored(&session);
}

#[test]
fn a_resize_with_the_help_open_leaves_the_scrollback_alone() {
    let mut session = started();
    session.wait_for("the route", resolved);
    session.send("\x1bOP");
    session.wait_for("the key help", |frame| {
        frame.rows[HINT].contains("esc close")
    });
    session.resize(50, 24);
    let frame = session.wait_for("a narrower frame", |frame| {
        frame.rows[0].chars().count() == 50 && frame.rows[HINT].contains("esc close")
    });
    assert!(frame.rows[0].starts_with("> type to filter"), "{frame}");
    session.send("\x1b");
    session.wait_for("the key help to close", |frame| {
        frame.rows[HINT].contains("esc quit")
    });
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    assert_restored(&session);
}

#[test]
fn a_two_row_terminal_shows_the_prompt_and_one_host_and_enter_connects_to_it() {
    let mut options = Options::new("ui");
    options.cols = 30;
    options.rows = 2;
    options.shell = &["$ sshpick"];
    let mut session = Session::start(options);
    let frame = session.wait_for("a two-row picker", |frame| {
        frame.alternate && frame.rows[1].ends_with("lab-a")
    });
    assert_eq!(
        frame.rows,
        ["> type to filter           5/5", "▌ LAB         lab-a"],
        "{frame}"
    );
    session.send("\x0e");
    session.wait_for("the next host", |frame| frame.rows[1].ends_with("lab-b"));
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert_eq!(frame.rows, ["ssh -- lab-b", ""], "{frame}");
    assert_eq!(
        session.scrollback(),
        ["$ sshpick", "> ssh -- lab-b", "COOKED"]
    );
}

#[test]
fn config_warnings_print_before_the_picker_and_are_still_there_after_it_closes() {
    let mut options = Options::new("warns");
    options.shell = SHELL;
    // The warning names the fixture's absolute path; a wide pty keeps it on one row wherever the
    // repository is checked out.
    options.cols = 240;
    let mut session = Session::start(options);
    let open = session.wait_for("the first frame", drawn);
    assert!(open.rows[0].starts_with("> type to filter"), "{open}");

    session.send("\x1bOP");
    let help = session.wait_for("the key help", |frame| {
        frame.rows[HINT].contains("esc close")
    });
    assert!(!help.has("Warnings"), "{help}");

    session.send("\x1b");
    session.wait_for("the key help to close", drawn);
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
        frame.rows[0].starts_with("> consul") && frame.rows[0].ends_with("2/5")
    });
}

#[test_case(Signal::SIGTERM ; "term")]
#[test_case(Signal::SIGHUP ; "hup")]
#[test_case(Signal::SIGINT ; "int")]
fn a_killed_picker_gives_the_shell_its_screen_back_and_dies_of_the_signal(signal: Signal) {
    let mut session = started();
    session.signal(signal);
    assert_eq!(session.exit(), Exit::Signal(signal));
    assert_restored(&session);
}

#[test]
fn a_picker_killed_with_the_help_open_gives_the_screen_back_too() {
    let mut session = started();
    session.send("\x1bOP");
    session.wait_for("the key help", |frame| {
        frame.rows[HINT].contains("esc close")
    });
    session.signal(Signal::SIGTERM);
    assert_eq!(session.exit(), Exit::Signal(Signal::SIGTERM));
    assert_restored(&session);
}

/// A question would stall on a terminal that never answers, as this pty does not. The only modes
/// set are the cursor, synchronized updates and the alternate screen.
#[test]
fn the_picker_never_asks_the_terminal_anything_or_sets_a_mode_of_its_own() {
    let mut session = started();
    session.wait_for("the route", resolved);
    session.send("\x1bOP");
    session.wait_for("the key help", |frame| {
        frame.rows[HINT].contains("esc close")
    });
    session.send("\x1b");
    session.wait_for("the key help to close", drawn);
    session.resize(50, 24);
    session.wait_for("a narrower frame", |frame| {
        frame.rows[0].chars().count() == 50 && drawn(frame)
    });
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));

    let bytes = session.output();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            assert_eq!(bytes.get(i + 1), Some(&b'['), "only CSI, at byte {i}");
            let mut j = i + 2;
            while j < bytes.len() && !(0x40..=0x7e).contains(&bytes[j]) {
                j += 1;
            }
            let (body, end) = (&bytes[i + 2..j], bytes[j]);
            let numbers = |body: &[u8]| body.iter().all(|b| b.is_ascii_digit() || *b == b';');
            let ok = match end {
                b'A' | b'B' | b'C' | b'D' | b'G' | b'H' | b'J' | b'K' | b'm' => numbers(body),
                b'h' | b'l' => [&b"?25"[..], b"?2026", b"?1049"].contains(&body),
                _ => false,
            };
            assert!(
                ok,
                "ESC[{}{} at byte {i}",
                String::from_utf8_lossy(body),
                end as char
            );
            i = j + 1;
        } else {
            assert!(
                matches!(bytes[i], b'\r' | b'\n') || bytes[i] >= 0x20,
                "control byte {:#x} at {i}",
                bytes[i]
            );
            i += 1;
        }
    }
}

#[test_case(&[], 2, "Error: 5 hosts to choose from, and the picker needs a terminal that moves the cursor (TERM=dumb)"; "several matches")]
#[test_case(&["zzz"], 1, "Error: no host matches \"zzz\""; "no match")]
#[test_case(&["lab-a"], 0, "ssh -- lab-a"; "one match still connects")]
fn a_dumb_terminal_never_gets_the_picker_or_an_escape_byte(
    args: &'static [&'static str],
    code: i32,
    said: &str,
) {
    let mut options = Options::new("ui");
    options.term = "dumb";
    options.cols = 120;
    options.args = args;
    let mut session = Session::start(options);
    assert_eq!(session.exit(), Exit::Code(code));
    let frame = session.frame();
    assert!(frame.has(said), "{frame}");
    assert!(!frame.has("type to filter"), "{frame}");
    assert!(!session.output().contains(&0x1b), "{frame}");
}

#[test]
fn doctor_on_a_terminal_sees_its_size_and_colour_and_notes_a_host_named_doctor() {
    let mut options = Options::new("named-doctor");
    options.args = &["doctor"];
    options.shell = &["$ sshpick doctor"];
    options.cols = 300;
    let mut session = Session::start(options);
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert_eq!(frame.rows[0], "$ sshpick doctor", "{frame}");
    assert_eq!(
        frame.rows[1], "> NOTE: \"doctor\" is also a host. Connect with `sshpick pick doctor`.",
        "{frame}"
    );
    assert_eq!(frame.rows[2], "[doctor] ok=5 warn=0 fail=0", "{frame}");
    assert!(
        frame.has(
            "[ok] terminal: 300x24, TERM=xterm-256color, colour on (stdout is a terminal); the picker can open here"
        ),
        "{frame}"
    );
    assert!(
        frame.has("[ok] resolve: ssh -G answered for doctor in "),
        "{frame}"
    );
    let bytes = String::from_utf8_lossy(&session.output()).into_owned();
    assert!(
        bytes.contains("\x1b[90m>\x1b[39m"),
        "the note's arrow is gray"
    );
    assert!(!bytes.contains("\x1b[33m"), "no warning, so no yellow");
}
