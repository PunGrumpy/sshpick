//! The real binary on a pty, read through a terminal emulator. The picker's terminal handling
//! (raw mode, the alternate screen, what is left behind) is reachable no other way.

#[path = "common/pty.rs"]
mod pty;

use nix::sys::signal::Signal;
use pty::{Exit, Frame, Options, Session};
use test_case::test_case;

const SHELL: &[&str] = &["$ git status", "clean", "$ keyway"];
/// The footer at 80 columns with an empty filter, from the first key to the last.
const KEYS: &str = "Keys ?    Connect ↵";
const CLOSE_KEYS: &str = "Close esc    Connect ↵";

fn started() -> Session {
    let mut options = Options::new("ui");
    options.shell = SHELL;
    let session = Session::start(options);
    session.wait_for("the first frame", drawn);
    session
}

const PROMPT: &str = "  Search host, ENV or what";

/// Where the cursor is on the prompt of a panel on `cols` columns, after `typed` characters: the
/// row under the border, and the query after the border, the margin and the strip's padding.
fn prompt_at(cols: u16, typed: u16) -> (u16, u16) {
    if cols >= 60 {
        let panel = (cols - 4).min(110);
        (2, (cols - panel) / 2 + 3 + typed)
    } else {
        (0, 2 + typed)
    }
}

/// What is inside the border, a line per row, as the picker's own tests see it. A terminal too
/// small for a border has the picker's rows, from the first that is not blank.
fn lines(frame: &Frame) -> Vec<String> {
    let find = |mark| frame.rows.iter().position(|row| row.contains(mark));
    if let Some((top, bottom)) = find('╭')
        .zip(find('╰'))
        .filter(|(top, bottom)| top < bottom)
    {
        return frame.rows[top + 1..bottom]
            .iter()
            .map(|row| inside(row))
            .collect();
    }
    let used = |row: &String| !row.is_empty();
    match (
        frame.rows.iter().position(used),
        frame.rows.iter().rposition(used),
    ) {
        (Some(first), Some(last)) => frame.rows[first..=last].to_vec(),
        _ => Vec::new(),
    }
}

fn inside(row: &str) -> String {
    let chars: Vec<char> = row.chars().collect();
    let edge = |c: &char| matches!(c, '│' | '├' | '┤');
    let (Some(from), Some(to)) = (chars.iter().position(edge), chars.iter().rposition(edge)) else {
        return row.to_owned();
    };
    let inner: String = chars.get(from + 1..to).unwrap_or_default().iter().collect();
    inner.trim_end().to_owned()
}

fn line(frame: &Frame, at: usize) -> String {
    lines(frame).get(at).cloned().unwrap_or_default()
}

/// The last line: the footer.
fn footer(frame: &Frame) -> String {
    lines(frame).pop().unwrap_or_default()
}

/// The footer is the last row inside the border and the cursor goes back to the prompt after the
/// frame, so once both hold the whole frame is on the screen.
fn drawn(frame: &Frame) -> bool {
    frame.alternate && footer(frame).contains("Connect ↵") && frame.cursor == prompt_at(80, 0)
}

/// One host matches, so the footer says `F1` for the key help.
fn one_match(frame: &Frame) -> bool {
    footer(frame).ends_with("Keys F1    Connect ↵")
}

fn resolved(frame: &Frame) -> bool {
    footer(frame).contains("ubuntu@")
}

/// The shell's screen as it was before keyway started, with nothing of the picker on it.
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
fn the_picker_is_a_panel_nearly_the_whole_screen_with_the_hint_row_under_a_rule() {
    let session = started();
    let frame = session.frame();
    let pad = " ".repeat(2);
    assert!(frame.rows[0].is_empty(), "a blank row above: {frame}");
    assert_eq!(
        frame.rows[1],
        format!("{pad}╭{}╮", "─".repeat(74)),
        "{frame}"
    );
    assert_eq!(
        frame.rows[22],
        format!("{pad}╰{}╯", "─".repeat(74)),
        "the panel is nearly the whole screen: {frame}"
    );
    assert!(frame.rows[23..].iter().all(String::is_empty), "{frame}");
    let inner = lines(&frame);
    assert_eq!(inner.len(), 20, "{frame}");
    assert!(
        inner[0].starts_with(PROMPT) && inner[0].ends_with("5 hosts"),
        "{frame}"
    );
    assert!(inner[1].is_empty(), "a blank row under the strip: {frame}");
    assert!(
        inner[2].starts_with("  lab-a")
            && inner[2].contains("LAB — personal VMs")
            && inner[2].ends_with("LAB"),
        "one line per host: {frame}"
    );
    assert!(
        inner[3].starts_with("  lab-b") && inner[3].contains("build box"),
        "{frame}"
    );
    assert!(inner[18].starts_with('─'), "the rule: {frame}");
    assert!(inner[19].ends_with(KEYS), "{frame}");
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
    session.wait_for("one match", one_match);
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert_eq!(
        frame.rows[..6],
        [
            "$ git status",
            "clean",
            "$ keyway",
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
    options.shell = &["$ keyway lab-a"];
    options.args = &["lab-a"];
    let mut session = Session::start(options);
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert_eq!(
        frame.rows[..4],
        ["$ keyway lab-a", "> ssh -- lab-a", "COOKED", "ssh -- lab-a"],
        "{frame}"
    );
    assert!(frame.rows[4..].iter().all(String::is_empty), "{frame}");
}

#[test]
fn enter_with_nothing_matched_says_so_and_exits_1() {
    let mut session = started();
    session.send("zzzz");
    session.wait_for("no match", |frame| {
        line(frame, 2) == "  No host matches \"zzzz\""
            && footer(frame).ends_with("Keys F1    Quit esc")
            && frame.cursor == prompt_at(80, 4)
    });
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(1));
    let frame = session.frame();
    assert_eq!(
        frame.rows[..5],
        [
            "$ git status",
            "clean",
            "$ keyway",
            "Error: no host matches \"zzzz\"",
            "Fix: list every host with `keyway ls`."
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
        "line 1", "line 2", "line 3", "line 4", "line 5", "line 6", "line 7", "line 8", "line 9",
        "line 10", "line 11", "line 12", "line 13", "line 14", "line 15", "line 16", "line 17",
        "line 18", "line 19", "line 20", "line 21", "line 22", "$ keyway",
    ];
    let mut options = Options::new("ui");
    options.shell = &HISTORY;
    let mut session = Session::start(options);
    let frame = session.wait_for("the first frame", drawn);
    assert!(line(&frame, 0).starts_with(PROMPT), "{frame}");
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

#[test_case(80, KEYS; "every key")]
#[test_case(33, "?     ↵"; "the keys alone")]
fn the_footer_is_the_last_row_of_the_panel_and_keeps_enter_and_esc(cols: u16, keys: &str) {
    let mut options = Options::new("ui");
    options.cols = cols;
    let mut session = Session::start(options);
    session.wait_for("the footer", |frame| {
        footer(frame).trim_start().ends_with(keys)
    });
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn the_route_is_on_the_footers_left_and_the_keys_on_its_right() {
    let mut session = started();
    let frame = session.wait_for("the route", resolved);
    let footer = footer(&frame);
    assert!(
        footer.starts_with("  ubuntu@192.0.2.11:22 via stg-bastion"),
        "{frame}"
    );
    assert!(footer.ends_with(KEYS), "{frame}");
    assert!(line(&frame, 2).starts_with("  lab-a"), "{frame}");
    assert!(line(&frame, 3).starts_with("  lab-b"), "{frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn a_host_ssh_cannot_resolve_shows_the_first_line_of_the_failure() {
    let mut options = Options::new("inspect");
    options.typed = "broken";
    let mut session = Session::start(options);
    let frame = session.wait_for("the failure", |frame| footer(frame).contains("failed"));
    let shown = footer(&frame);
    assert!(
        shown.starts_with("  ssh -G failed: /home/me/.ssh/config.d/10-stagin…"),
        "the route is cut past the columns the footer keeps for it: {frame}"
    );
    assert!(!frame.has("terminating"), "only the first line: {frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn the_key_help_takes_rows_from_the_list_without_moving_the_panel() {
    let mut options = Options::new("ui");
    options.cols = 120;
    options.rows = 30;
    let mut session = Session::start(options);
    session.wait_for("the route", resolved);
    let before = session.frame();
    session.send("\x1bOP");
    let frame = session.wait_for("the key help", |frame| footer(frame).contains("Close esc"));
    assert!(
        frame.has("quit (exit 130); Esc closes the help first")
            && frame.has("toggle help; ? only on an empty filter")
            && !frame.rows.iter().any(|row| row.contains('…')),
        "no key row is cut: {frame}"
    );
    let border = |frame: &Frame| {
        frame
            .rows
            .iter()
            .filter(|row| row.contains(['╭', '╰', '├']))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        border(&frame),
        border(&before),
        "the help does not move the panel: {frame}"
    );

    session.send("\x1b");
    let frame = session.wait_for("the key help to close", |frame| {
        footer(frame).contains("Keys")
    });
    assert_eq!(border(&frame), border(&before), "{frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn a_wide_terminal_centres_the_same_box_and_enter_leaves_none_of_it() {
    let mut options = Options::new("ui");
    options.shell = SHELL;
    options.cols = 120;
    let mut session = Session::start(options);
    let frame = session.wait_for("the route", resolved);
    assert!(
        footer(&frame).starts_with("  ubuntu@192.0.2.11:22 via stg-bastion"),
        "{frame}"
    );
    assert!(footer(&frame).contains("Quit esc"), "{frame}");
    assert_eq!(
        frame.rows[1],
        format!("{}╭{}╮", " ".repeat(5), "─".repeat(108)),
        "110 columns, centred: {frame}"
    );
    assert_eq!(
        frame.rows.iter().filter(|row| row.contains('└')).count(),
        0,
        "the route is in the footer, not under a host: {frame}"
    );
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert_eq!(
        frame.rows[..6],
        [
            "$ git status",
            "clean",
            "$ keyway",
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
fn the_key_help_takes_rows_and_esc_closes_it_before_the_picker(open: &str) {
    let mut session = started();
    session.send(open);
    let frame = session.wait_for("the key help", |frame| {
        footer(frame).ends_with(CLOSE_KEYS) && footer(frame).contains("ubuntu@")
    });
    assert!(line(&frame, 0).starts_with(PROMPT), "{frame}");
    assert!(
        line(&frame, 2).starts_with("  lab-a"),
        "the list is still there: {frame}"
    );
    assert!(
        footer(&frame).starts_with("  ubuntu@"),
        "the route is on the footer: {frame}"
    );
    assert!(line(&frame, 7).is_empty(), "{frame}");
    assert!(line(&frame, 8).starts_with("  Enter"), "{frame}");
    assert!(
        frame.has("toggle help; ? only on an empty filter"),
        "{frame}"
    );
    assert!(footer(&frame).ends_with(CLOSE_KEYS), "{frame}");
    let corners = frame.rows.iter().filter(|row| row.contains('╭')).count();
    assert!(
        !frame.has("Keybinds") && corners == 1,
        "not a popup, a section of the one panel: {frame}"
    );

    session.send("consul");
    let frame = session.wait_for("the filter to apply with the help open", |frame| {
        footer(frame).contains("Close esc")
            && line(frame, 0).starts_with("  consul")
            && line(frame, 3).starts_with("  newstg-consul2")
            && frame.cursor == prompt_at(80, 6)
    });
    assert!(
        frame.has("toggle help; ? only on an empty filter"),
        "{frame}"
    );

    session.send("\x1b");
    let frame = session.wait_for("the key help to close, not the picker", |frame| {
        footer(frame).contains("Keys") && frame.cursor == prompt_at(80, 6)
    });
    assert!(frame.alternate && !frame.has("toggle help"), "{frame}");
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    assert_restored(&session);
}

#[test]
fn a_short_terminal_points_at_pick_help_for_the_keys_it_cannot_show() {
    let mut options = Options::new("ui");
    options.rows = 16;
    let mut session = Session::start(options);
    let closed = |frame: &Frame| footer(frame).contains("Keys") && frame.cursor == (2, 5);
    session.wait_for("the first frame", closed);
    session.send("\x1bOP");
    let frame = session.wait_for("the key help", |frame| {
        footer(frame).contains("Close esc") && frame.cursor == (2, 5)
    });
    assert_eq!(line(&frame, 9), "  +7 more · keyway pick --help", "{frame}");
    assert!(line(&frame, 6).starts_with("  Enter"), "{frame}");
    session.send("\x1b");
    session.wait_for("the key help to close", closed);
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
}

#[test]
fn a_resize_redraws_one_picker_in_the_new_size() {
    let mut session = started();
    session.wait_for("the route", resolved);
    //     cols rows  where the cursor goes: row, column
    for (cols, rows, cursor) in [
        (50, 24, (0, 2)),
        (120, 24, (2, 8)),
        (40, 24, (0, 2)),
        (80, 12, (2, 5)),
    ] {
        session.resize(cols, rows);
        let frame = session.wait_for(&format!("a frame at {cols}x{rows}"), |frame| {
            footer(frame).contains('↵') && frame.cursor == cursor
        });
        let prompts = frame
            .rows
            .iter()
            .filter(|row| row.contains("Search host"))
            .count();
        let hints = frame.rows.iter().filter(|row| row.contains('↵')).count();
        assert_eq!((prompts, hints), (1, 1), "{frame}");
        assert!(footer(&frame).contains("ubuntu@"), "{frame}");
        assert_eq!(
            frame.has("╭"),
            cols >= 60 && rows >= 10,
            "a border where there is room for one: {frame}"
        );
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
    session.wait_for("the key help", |frame| footer(frame).contains("Close esc"));
    session.resize(50, 24);
    let frame = session.wait_for("a narrower frame", |frame| {
        footer(frame).contains("esc") && frame.cursor == (0, 2)
    });
    assert!(line(&frame, 0).starts_with(PROMPT), "{frame}");
    session.send("\x1b");
    session.wait_for("the key help to close", |frame| {
        footer(frame).contains('↵') && !footer(frame).contains("esc") && frame.cursor == (0, 2)
    });
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    assert_restored(&session);
}

#[test]
fn a_two_row_terminal_shows_the_prompt_and_the_selected_host_and_enter_connects_to_it() {
    let mut options = Options::new("ui");
    options.cols = 30;
    options.rows = 2;
    options.shell = &["$ keyway"];
    let mut session = Session::start(options);
    let frame = session.wait_for("a two-row picker", |frame| {
        frame.alternate && frame.rows[1].contains("lab-a")
    });
    assert!(frame.rows[0].contains("Search host"), "{frame}");
    assert!(
        frame.rows[1].contains("lab-a"),
        "one line per host: {frame}"
    );
    session.send("\x0e");
    session.wait_for("the next host", |frame| frame.rows[1].contains("lab-b"));
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert_eq!(frame.rows, ["ssh -- lab-b", ""], "{frame}");
    assert_eq!(
        session.scrollback(),
        ["$ keyway", "> ssh -- lab-b", "COOKED"]
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
    let first = |frame: &Frame| {
        frame.alternate && footer(frame).contains("Keys") && frame.cursor == prompt_at(240, 0)
    };
    let open = session.wait_for("the first frame", first);
    assert!(line(&open, 0).starts_with(PROMPT), "{open}");

    session.send("\x1bOP");
    let help = session.wait_for("the key help", |frame| footer(frame).contains("Close esc"));
    assert!(!help.has("Warnings"), "{help}");

    session.send("\x1b");
    session.wait_for("the key help to close", first);
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
        line(frame, 0).starts_with("  consul") && line(frame, 3).starts_with("  newstg-consul2")
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
    session.wait_for("the key help", |frame| footer(frame).contains("Close esc"));
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
    session.wait_for("the key help", |frame| footer(frame).contains("Close esc"));
    session.send("\x1b");
    session.wait_for("the key help to close", drawn);
    session.resize(50, 24);
    session.wait_for("a narrower frame", |frame| {
        footer(frame).contains('↵') && frame.cursor == (0, 2)
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
    assert!(!frame.has("Search host"), "{frame}");
    assert!(!session.output().contains(&0x1b), "{frame}");
}

#[test]
fn doctor_on_a_terminal_sees_its_size_and_colour_and_notes_a_host_named_doctor() {
    let mut options = Options::new("named-doctor");
    options.args = &["doctor"];
    options.shell = &["$ keyway doctor"];
    options.cols = 300;
    let mut session = Session::start(options);
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert_eq!(frame.rows[0], "$ keyway doctor", "{frame}");
    assert_eq!(
        frame.rows[1], "> NOTE: \"doctor\" is also a host. Connect with `keyway pick doctor`.",
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

const RECENT: &[(&str, u64)] = &[
    ("newstg-consul2", 2 * 3600 + 30),
    ("lab-b", 3 * 86_400 + 30),
];

fn recorded(session: &Session) -> Vec<(u64, String)> {
    let history = session.history().unwrap_or_default();
    history
        .lines()
        .filter_map(|line| {
            let (time, host) = line.split_once('\t')?;
            Some((time.parse().ok()?, host.to_owned()))
        })
        .collect()
}

fn recorded_hosts(session: &Session) -> Vec<String> {
    recorded(session)
        .into_iter()
        .map(|(_, host)| host)
        .collect()
}

#[test]
fn enter_records_the_host_it_ran_ssh_for() {
    let mut session = started();
    assert_eq!(session.history(), None, "nothing before a connection");
    session.send("'lab-b");
    session.wait_for("one match", one_match);
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    assert_eq!(
        recorded_hosts(&session),
        ["lab-b"],
        "{:?}",
        session.history()
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    assert!(now.abs_diff(recorded(&session)[0].0) < 60);
}

#[test_case(&["lab-a"], "lab-a"; "an exact name")]
#[test_case(&["bastion"], "newstg-bastion"; "the one host a query matches")]
fn a_connection_without_the_picker_is_recorded_at_a_terminal_too(
    args: &'static [&str],
    host: &str,
) {
    let mut options = Options::new("ui");
    options.args = args;
    let mut session = Session::start(options);
    assert_eq!(session.exit(), Exit::Code(0));
    assert_eq!(recorded_hosts(&session), [host], "{:?}", session.history());
}

#[test_case(&[]; "with no history")]
#[test_case(RECENT; "with a history to leave alone")]
fn non_interactive_at_a_terminal_reads_and_records_nothing(history: &'static [(&str, u64)]) {
    let mut options = Options::new("ui");
    options.args = &["--non-interactive", "lab-a"];
    options.history = history;
    let mut session = Session::start(options);
    let seeded = session.history();
    assert_eq!(session.exit(), Exit::Code(0));
    assert!(session.frame().has("ssh -- lab-a"));
    assert_eq!(session.history(), seeded);
    assert_eq!(session.state().exists(), !history.is_empty());
}

#[test]
fn the_host_used_last_is_the_first_row_and_one_enter_reconnects_to_it() {
    let mut options = Options::new("ui");
    options.history = RECENT;
    let mut session = Session::start(options);
    let frame = session.wait_for("the first frame", drawn);
    assert!(line(&frame, 1).is_empty(), "{frame}");
    assert!(
        line(&frame, 2).starts_with("  newstg-consul2")
            && line(&frame, 2).ends_with("2h  NEW STAGING"),
        "{frame}"
    );
    assert!(
        line(&frame, 3).starts_with("  lab-b")
            && line(&frame, 3).contains("3d")
            && line(&frame, 3).ends_with("  LAB"),
        "LAST before ENV on the same line: {frame}"
    );
    assert!(line(&frame, 4).starts_with("  lab-a"), "{frame}");

    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    assert!(session.frame().has("ssh -- newstg-consul2"));
    assert_eq!(
        recorded_hosts(&session),
        ["newstg-consul2", "lab-b", "newstg-consul2"]
    );
}

#[test]
fn ls_at_a_terminal_ignores_the_history() {
    let mut options = Options::new("ui");
    options.args = &["ls"];
    options.history = RECENT;
    let mut session = Session::start(options);
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert!(!frame.has("LAST") && !frame.has("2h"), "{frame}");
    let hosts: Vec<_> = frame
        .rows
        .iter()
        .skip(1)
        .filter_map(|row| row.split_whitespace().nth(1))
        .collect();
    assert_eq!(hosts[..2], ["HOST", "lab-a"], "{frame}");
}

#[test]
fn a_history_that_cannot_be_saved_is_one_warning_and_ssh_still_starts() {
    let mut options = Options::new("ui");
    options.args = &["lab-a"];
    options.unwritable = true;
    options.cols = 200;
    let mut session = Session::start(options);
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    let warnings = frame
        .rows
        .iter()
        .filter(|row| row.starts_with("! cannot save recent hosts to "))
        .count();
    assert_eq!(warnings, 1, "{frame}");
    assert!(frame.has("ssh -- lab-a"), "{frame}");
    assert_eq!(session.history(), None);
}

/// A file per test under cargo's scratch directory, for stdout that is not the terminal.
fn stdout_file(name: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("picker-{name}-{}.out", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn printing(file: &std::path::Path) -> Session {
    let mut options = Options::new("ui");
    options.shell = SHELL;
    options.args = &["--print"];
    options.stdout_file = Some(file.to_owned());
    let session = Session::start(options);
    session.wait_for("the first frame", drawn);
    session
}

#[test]
fn print_draws_on_the_terminal_while_stdout_gets_only_the_host() {
    let file = stdout_file("print");
    let mut session = printing(&file);
    session.send("lab-a");
    session.wait_for("one match", one_match);
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    assert_restored(&session);
    assert!(
        !String::from_utf8_lossy(&session.output()).contains("ssh --"),
        "ssh never ran and no `>` line was printed"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "lab-a\n");
    assert_eq!(
        recorded_hosts(&session),
        ["lab-a"],
        "a printed pick is a recent host"
    );
    std::fs::remove_file(&file).unwrap();
}

#[test]
fn print_cancelled_exits_130_with_stdout_empty() {
    let file = stdout_file("cancel");
    let mut session = printing(&file);
    session.send("\x1b");
    assert_eq!(session.exit(), Exit::Code(130));
    assert_restored(&session);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "");
    std::fs::remove_file(&file).unwrap();
}

#[test]
fn print_killed_restores_the_terminal_it_drew_on() {
    let file = stdout_file("killed");
    let mut session = printing(&file);
    session.signal(Signal::SIGTERM);
    assert_eq!(session.exit(), Exit::Signal(Signal::SIGTERM));
    assert_restored(&session);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "");
    std::fs::remove_file(&file).unwrap();
}

#[test]
fn print_on_a_terminal_prints_the_host_after_the_shell_lines() {
    let mut options = Options::new("ui");
    options.shell = SHELL;
    options.args = &["--print"];
    let mut session = Session::start(options);
    session.wait_for("the first frame", drawn);
    session.send("build");
    session.wait_for("one match", one_match);
    session.send("\r");
    assert_eq!(session.exit(), Exit::Code(0));
    let frame = session.frame();
    assert!(!frame.alternate, "{frame}");
    assert_eq!(
        frame.rows[..4],
        ["$ git status", "clean", "$ keyway", "lab-b"],
        "{frame}"
    );
    assert!(frame.rows[4..].iter().all(String::is_empty), "{frame}");
    assert!(session.cooked());
}
