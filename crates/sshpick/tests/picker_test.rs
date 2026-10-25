//! The real binary on a pty, read through a terminal emulator. The picker's terminal handling
//! (raw mode, the block under the command line, what is left behind) is reachable no other way.

#[path = "common/pty.rs"]
mod pty;

use nix::sys::signal::Signal;
use pty::{Exit, Frame, Options, Session};
use test_case::test_case;

const SHELL: &[&str] = &["$ git status", "clean", "$ sshpick"];
/// The block opens on the row under the command line.
const TOP: usize = 3;
/// The `ui` fixture's five hosts make a block of nine rows: prompt, header, hosts, route, hint.
const HINT: usize = TOP + 8;

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
    frame.rows[HINT].contains("esc quit") && frame.cursor == (3, 2)
}

fn resolved(frame: &Frame) -> bool {
    frame.rows[TOP + 7].contains("ubuntu@")
}

fn assert_restored(session: &Session) {
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert!(!frame.cursor_hidden, "{frame}");
    assert_eq!(frame.cursor, (3, 0), "{frame}");
    assert_eq!(frame.rows[..TOP], SHELL[..], "{frame}");
    assert!(frame.rows[TOP..].iter().all(String::is_empty), "{frame}");
    assert!(
        session.scrollback().is_empty(),
        "{:?}",
        session.scrollback()
    );
    assert!(session.cooked());
}

#[test]
fn the_block_opens_under_the_command_line_and_the_shell_stays_above_it() {
    let session = started();
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert_eq!(frame.rows[..TOP], SHELL[..], "{frame}");
    assert!(
        frame.rows[TOP].starts_with("> type to filter") && frame.rows[TOP].ends_with("5/5"),
        "{frame}"
    );
    assert_eq!(
        frame.rows[TOP + 1],
        "  ENV         HOST           WHAT",
        "{frame}"
    );
    assert!(
        frame.rows[TOP + 2].starts_with("▌ LAB         lab-a"),
        "{frame}"
    );
    assert_eq!(
        frame.rows[HINT], "  ↑↓ move   enter ssh   esc quit   ? keys",
        "{frame}"
    );
    assert!(
        frame.rows[HINT + 1..].iter().all(String::is_empty),
        "nine rows and no more: {frame}"
    );
    assert!(session.scrollback().is_empty());
    assert!(!session.cooked());
}

#[test]
fn esc_exits_130_and_leaves_nothing_behind() {
    let mut session = started();
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    assert_restored(&session);
}

#[test]
fn enter_says_which_host_runs_where_the_block_was_then_ssh_starts_on_a_cooked_terminal() {
    let mut session = started();
    session.send("lab-a");
    session.wait_for("one match", |frame| frame.rows[TOP].ends_with("1/5"));
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert_eq!(
        frame.rows[..TOP + 3],
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
    assert!(
        frame.rows[TOP + 3..].iter().all(String::is_empty),
        "{frame}"
    );
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
        frame.rows[TOP + 2] == "  no matching hosts" && frame.rows[HINT] == "  esc quit   F1 keys"
    });
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(1));
    let frame = session.frame();
    assert_eq!(
        frame.rows[..TOP + 2],
        [
            "$ git status",
            "clean",
            "$ sshpick",
            "Error: no host matches \"zzzz\"",
            "Fix: list every host with `sshpick ls`."
        ],
        "{frame}"
    );
    assert!(
        frame.rows[TOP + 2..].iter().all(String::is_empty),
        "{frame}"
    );
    assert_eq!(frame.cursor, (5, 0), "{frame}");
    assert!(session.cooked());
}

#[test]
fn started_at_the_bottom_of_the_screen_the_history_scrolls_up_and_survives() {
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
    let frame = session.wait_for("the first frame", |frame| {
        frame.rows[23].contains("esc quit") && frame.cursor == (15, 2)
    });
    assert_eq!(frame.rows[..15], HISTORY[8..], "{frame}");
    assert!(frame.rows[15].starts_with("> type to filter"), "{frame}");
    assert_eq!(session.scrollback(), HISTORY[..8]);

    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    let frame = session.frame();
    assert_eq!(frame.rows[..15], HISTORY[8..], "{frame}");
    assert!(frame.rows[15..].iter().all(String::is_empty), "{frame}");
    assert_eq!(frame.cursor, (15, 0), "{frame}");
    assert_eq!(session.scrollback(), HISTORY[..8]);
}

#[test]
fn a_picker_opened_on_the_top_row_leaves_the_scrollback_alone() {
    let mut session = Session::start(Options::new("ui"));
    session.wait_for("the first frame", |frame| {
        frame.rows[8].contains("esc quit") && frame.cursor == (0, 2)
    });
    session.send("\x1bOP");
    session.wait_for("the key help", |frame| frame.rows[19].contains("esc close"));
    session.send("\x1b");
    session.wait_for("the key help to close", |frame| {
        frame.rows[8].contains("esc quit") && frame.rows[9..].iter().all(String::is_empty)
    });
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    let frame = session.frame();
    assert!(frame.rows.iter().all(String::is_empty), "{frame}");
    assert_eq!(frame.cursor, (0, 0), "{frame}");
    assert!(
        session.scrollback().is_empty(),
        "{:?}",
        session.scrollback()
    );
}

#[test_case(80, "  ↑↓ move   enter ssh   esc quit   ? keys"; "every key")]
#[test_case(30, "  enter ssh   esc quit"; "no movement or help")]
#[test_case(20, "  enter esc"; "bare key names")]
fn the_hint_row_is_the_last_row_of_the_block_and_keeps_enter_and_esc(cols: u16, row: &str) {
    let mut options = Options::new("ui");
    options.cols = cols;
    let mut session = Session::start(options);
    let frame = session.wait_for("the hint row", |frame| frame.rows[8] == row);
    assert!(frame.rows[9..].iter().all(String::is_empty), "{frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn the_route_row_says_where_the_selected_host_goes_and_the_hint_row_stays_last() {
    let mut session = started();
    let frame = session.wait_for("the route", resolved);
    assert_eq!(
        frame.rows[TOP + 7],
        "  → ubuntu@192.0.2.11:22 via stg-bastion",
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
    options.cols = 120;
    options.typed = "broken";
    let mut session = Session::start(options);
    // Four hosts: prompt, header, four host rows, then the route on row 6.
    let frame = session.wait_for("the failure", |frame| frame.rows[6].contains("failed"));
    assert_eq!(
        frame.rows[6],
        "  → ssh -G failed: /home/me/.ssh/config.d/10-staging.conf line 6: Bad configuration option: bogus",
        "{frame}"
    );
    assert!(!frame.has("terminating"), "only the first line: {frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test_case("\x1bOP"; "f1")]
#[test_case("?"; "question mark on an empty filter")]
fn the_key_help_grows_the_block_in_place_and_closing_it_gives_the_rows_back(open: &str) {
    let mut session = started();
    session.send(open);
    let frame = session.wait_for("the key help", |frame| {
        frame.rows[TOP + 19].contains("esc close")
    });
    assert_eq!(
        frame.rows[..TOP],
        SHELL[..],
        "nothing above the block moved: {frame}"
    );
    assert!(frame.rows[TOP].starts_with("> type to filter"), "{frame}");
    assert!(
        frame.rows[TOP + 2].starts_with("▌ LAB         lab-a"),
        "the list is still there: {frame}"
    );
    assert!(frame.rows[TOP + 8].is_empty(), "{frame}");
    assert!(frame.rows[TOP + 9].starts_with("  Enter"), "{frame}");
    assert!(frame.has("toggle this help"), "{frame}");
    assert_eq!(
        frame.rows[TOP + 19],
        "  ↑↓ move   enter ssh   esc close",
        "{frame}"
    );
    assert!(frame.rows[TOP + 20].is_empty(), "{frame}");
    assert!(
        !frame.has("Keybinds") && !frame.has("┘"),
        "not a popup: {frame}"
    );

    session.send("consul");
    let frame = session.wait_for("the filter to apply with the help open", |frame| {
        frame.rows[TOP + 19].contains("esc close")
            && frame.rows[TOP].starts_with("> consul")
            && frame.rows[TOP].ends_with("2/5")
            && frame.rows[TOP + 3] == "  NEW STAGING newstg-consul2"
    });
    assert!(frame.has("toggle this help"), "{frame}");

    session.send("\x1b");
    let frame = session.wait_for("the key help to close, not the picker", |frame| {
        frame.rows[HINT].contains("esc quit") && frame.rows[HINT + 1..].iter().all(String::is_empty)
    });
    assert_eq!(frame.rows[..TOP], SHELL[..], "{frame}");
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
        frame.rows[7].contains("esc quit")
    });
    session.send("\x1bOP");
    let frame = session.wait_for("the key help", |frame| frame.rows[10].contains("esc close"));
    assert_eq!(frame.rows[9], "  +8 more · sshpick pick --help", "{frame}");
    assert!(frame.rows[7].starts_with("  Enter"), "{frame}");
    assert!(
        frame.rows[11].is_empty(),
        "the block never takes the last row: {frame}"
    );
    session.send("\x1b");
    session.wait_for("the key help to close", |frame| {
        frame.rows[7].contains("esc quit") && frame.rows[8..].iter().all(String::is_empty)
    });
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn a_resize_leaves_exactly_one_block_and_the_shell_above_it() {
    let mut session = started();
    session.wait_for("the route", resolved);
    for (cols, rows) in [(50, 24), (120, 24), (40, 24), (80, 12)] {
        session.resize(cols, rows);
        // At 12 rows the block is 8: one host fewer.
        let hint = if rows == 12 { HINT - 1 } else { HINT };
        let frame = session.wait_for(&format!("a frame at {cols}x{rows}"), |frame| {
            frame.rows[TOP].chars().count() == usize::from(cols)
                && frame.rows[TOP].ends_with("5/5")
                && frame.rows[hint].contains("esc quit")
                && frame.cursor == (3, 2)
        });
        assert_eq!(frame.rows[..TOP], SHELL[..], "{frame}");
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
        assert!(frame.rows[hint - 1].contains("ubuntu@"), "{frame}");
        assert!(
            frame.rows[hint + 1..].iter().all(String::is_empty),
            "{frame}"
        );
    }
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    let frame = session.frame();
    assert_eq!(frame.rows[..TOP], SHELL[..], "{frame}");
    assert!(frame.rows[TOP..].iter().all(String::is_empty), "{frame}");
    assert_eq!(frame.cursor, (3, 0), "{frame}");
}

#[test]
fn a_two_row_terminal_shows_the_selection_on_the_prompt_row_and_enter_connects_to_it() {
    let mut options = Options::new("ui");
    options.cols = 30;
    options.rows = 2;
    options.shell = &["$ sshpick"];
    let mut session = Session::start(options);
    let frame = session.wait_for("a one-row block", |frame| frame.rows[1].ends_with("lab-a"));
    assert_eq!(
        frame.rows,
        ["$ sshpick", "> type to filter         lab-a"],
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
fn config_warnings_print_above_the_block_and_are_still_there_after_it_closes() {
    let mut options = Options::new("warns");
    options.shell = SHELL;
    // The warning names the fixture's absolute path; a wide pty keeps it on one row wherever the
    // repository is checked out.
    options.cols = 240;
    let mut session = Session::start(options);
    // Two hosts: a block of six rows under the warning.
    let open = session.wait_for("the first frame", |frame| {
        frame.rows[9].contains("esc quit")
    });
    assert_eq!(open.rows[..TOP], SHELL[..], "{open}");
    assert!(
        open.rows[3].starts_with("! ") && open.rows[3].ends_with("skipped, include cycle"),
        "{open}"
    );
    assert!(open.rows[4].starts_with("> type to filter"), "{open}");

    session.send("\x1bOP");
    let help = session.wait_for("the key help", |frame| frame.rows[20].contains("esc close"));
    assert!(!help.has("Warnings"), "{help}");
    let warned = help.rows.iter().filter(|row| row.contains("include cycle"));
    assert_eq!(warned.count(), 1, "{help}");

    session.send("\x1b");
    session.wait_for("the key help to close", |frame| {
        frame.rows[9].contains("esc quit") && frame.rows[10..].iter().all(String::is_empty)
    });
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert_eq!(frame.rows[..TOP], SHELL[..], "{frame}");
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
fn a_killed_picker_erases_the_block_and_dies_of_the_signal(signal: Signal) {
    let mut session = started();
    session.signal(signal);
    assert_eq!(session.exit(), Exit::Signal(signal));
    assert_restored(&session);
}

#[test]
fn a_picker_killed_with_the_help_open_erases_all_of_it() {
    let mut session = started();
    session.send("\x1bOP");
    session.wait_for("the key help", |frame| {
        frame.rows[TOP + 19].contains("esc close")
    });
    session.signal(Signal::SIGTERM);
    assert_eq!(session.exit(), Exit::Signal(Signal::SIGTERM));
    assert_restored(&session);
}

/// A question would stall on a terminal that never answers, as this pty does not; an erase to the
/// end of the screen is what tmux copies into its scrollback.
#[test]
fn the_picker_never_asks_the_terminal_anything_or_erases_to_the_end_of_it() {
    let mut session = started();
    session.wait_for("the route", resolved);
    session.send("\x1bOP");
    session.wait_for("the key help", |frame| {
        frame.rows[TOP + 19].contains("esc close")
    });
    session.send("\x1b");
    session.wait_for("the key help to close", |frame| {
        frame.rows[HINT].contains("esc quit") && frame.rows[HINT + 1..].iter().all(String::is_empty)
    });
    session.resize(50, 24);
    session.wait_for("a narrower frame", |frame| {
        frame.rows[TOP].chars().count() == 50 && drawn(frame)
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
            let ok = match end {
                b'A' | b'G' => !body.is_empty() && body.iter().all(u8::is_ascii_digit),
                b'K' => body.is_empty() || body == b"2",
                b'm' => body.iter().all(|b| b.is_ascii_digit() || *b == b';'),
                b'h' | b'l' => body == b"?25" || body == b"?2026",
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

/// The rows of a 20-row help at 80 columns would rewrap into 33 at 50 on a terminal that rewraps
/// long lines, more than the screen has, so all of them are erased. On one that does not, such as
/// vt100, that scrolls the shell's lines into the scrollback: kept, but no longer on the screen.
#[test]
fn narrowing_with_the_help_open_on_a_short_screen_keeps_the_shell_in_the_scrollback() {
    let mut session = started();
    session.wait_for("the route", resolved);
    session.send("\x1bOP");
    session.wait_for("the key help", |frame| {
        frame.rows[TOP + 19].contains("esc close")
    });
    session.resize(50, 24);
    let frame = session.wait_for("a narrower frame", |frame| {
        frame.rows[0].chars().count() == 50 && frame.rows[19].contains("esc close")
    });
    assert!(frame.rows[0].starts_with("> type to filter"), "{frame}");
    assert!(frame.rows[20..].iter().all(String::is_empty), "{frame}");
    let scrollback = session.scrollback();
    assert_eq!(scrollback[..TOP], SHELL[..], "{scrollback:#?}");
    assert!(
        scrollback[TOP..].iter().all(String::is_empty),
        "nothing of the block: {scrollback:#?}"
    );
    session.send("\x1b");
    session.wait_for("the key help to close", |frame| {
        frame.rows[8].contains("esc quit") && frame.rows[9..].iter().all(String::is_empty)
    });
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    let frame = session.frame();
    assert!(frame.rows.iter().all(String::is_empty), "{frame}");
    assert_eq!(frame.cursor, (0, 0), "{frame}");
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
