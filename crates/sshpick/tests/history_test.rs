//! Without a terminal sshpick neither reads nor writes the history, so what a script sees is a
//! function of the config and the query. The one exception is `ls --recent`, which a script asks
//! for by name and which only reads. The picker's half is in `picker_test.rs`, on a pty.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use common::{sshpick, stderr};
use test_case::test_case;

struct State(PathBuf);

impl State {
    fn new(name: &str) -> State {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        State(std::env::temp_dir().join(format!(
            "sshpick-history-test-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }

    fn seeded(name: &str) -> State {
        State::with(name, "1794556700\tnewstg-consul2\n1794556790\tlab-b\n")
    }

    fn with(name: &str, lines: &str) -> State {
        let state = State::new(name);
        let dir = state.0.join("sshpick");
        fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("cannot create {dir:?}: {error}"));
        fs::write(dir.join("history"), lines)
            .unwrap_or_else(|error| panic!("cannot seed the history: {error}"));
        state
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for State {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(state: &State, args: &[&str]) -> (Option<i32>, Vec<u8>, String) {
    let assert = sshpick("ui")
        .env("XDG_STATE_HOME", state.path())
        .args(args)
        .assert();
    let out = assert.get_output();
    (out.status.code(), out.stdout.clone(), stderr(&assert))
}

#[test_case(&["lab-a"]; "an exact name")]
#[test_case(&["bastion"]; "the one host a query matches")]
#[test_case(&["--non-interactive", "lab-a"]; "non-interactive")]
#[test_case(&["consul"]; "several matches")]
fn a_run_without_a_terminal_leaves_no_state_behind(args: &[&str]) {
    let state = State::new("absent");
    run(&state, args);
    assert!(!state.path().exists(), "{args:?}");
}

#[test_case(&["ls"]; "ls")]
#[test_case(&["ls", "consul"]; "ls with a query")]
#[test_case(&["ls", "--json"]; "ls json")]
#[test_case(&["--list"]; "the older spelling")]
#[test_case(&["inspect", "lab-a", "--json"]; "inspect json")]
#[test_case(&["inspect", "consul", "--json"]; "inspect with several candidates")]
#[test_case(&["consul", "--json"]; "pick with several candidates")]
#[test_case(&["--non-interactive", "consul"]; "pick refused")]
fn a_history_changes_not_one_byte_a_script_sees(args: &[&str]) {
    let none = run(&State::new("none"), args);
    let seeded = State::seeded("seeded");
    let with = run(&seeded, args);
    assert_eq!(with, none, "{args:?}");
    let history = fs::read_to_string(seeded.path().join("sshpick/history")).unwrap_or_default();
    assert_eq!(history.lines().count(), 2, "nothing was recorded");
}

fn hosts(state: &State, args: &[&str]) -> Vec<String> {
    let (code, stdout, _) = run(state, args);
    assert!(matches!(code, Some(0 | 1)), "{args:?}: {code:?}");
    let text = String::from_utf8_lossy(&stdout).into_owned();
    if args.contains(&"--json") {
        let document: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let names = document["hosts"].as_array().into_iter().flatten();
        return names
            .filter_map(|host| host["host"].as_str().map(str::to_owned))
            .collect();
    }
    text.lines()
        .filter_map(|line| line.split('\t').nth(1).map(str::to_owned))
        .collect()
}

const CONFIG_ORDER: [&str; 5] = [
    "lab-a",
    "lab-b",
    "newstg-consul1",
    "newstg-consul2",
    "newstg-bastion",
];
const RECENT_ORDER: [&str; 5] = [
    "lab-b",
    "newstg-consul2",
    "lab-a",
    "newstg-consul1",
    "newstg-bastion",
];

#[test_case(&["ls", "--recent"]; "tab-separated rows")]
#[test_case(&["ls", "--recent", "--json"]; "json")]
#[test_case(&["ls", "--recent", "-l"]; "the older spelling")]
#[test_case(&["--non-interactive", "ls", "--recent"]; "non-interactive")]
fn recent_lists_the_hosts_you_used_first_most_recent_first_then_the_config(args: &[&str]) {
    let state = State::seeded("recent");
    assert_eq!(hosts(&state, args), RECENT_ORDER, "{args:?}");
}

#[test_case(&["ls"]; "tab-separated rows")]
#[test_case(&["ls", "--json"]; "json")]
fn a_plain_ls_keeps_the_config_order_whatever_the_history_holds(args: &[&str]) {
    let state = State::seeded("plain");
    assert_eq!(hosts(&state, args), CONFIG_ORDER, "{args:?}");
}

#[test]
fn a_query_with_equal_matches_lists_the_one_used_last_first_under_recent() {
    let state = State::with("tie", "1794556700\tnewstg-consul1\n");
    let config = ["newstg-consul2", "newstg-bastion", "newstg-consul1"];
    assert_eq!(hosts(&state, &["ls", "newstg"]), config);
    assert_eq!(
        hosts(&state, &["ls", "--recent", "newstg"]),
        ["newstg-consul1", "newstg-consul2", "newstg-bastion"]
    );
}

#[test_case(&["ls", "--recent"]; "all hosts")]
#[test_case(&["ls", "--recent", "--json"]; "json")]
#[test_case(&["ls", "--recent", "lab"]; "a query")]
#[test_case(&["ls", "--recent", "zzz"]; "no match")]
fn recent_with_no_history_is_plain_ls_to_the_byte(args: &[&str]) {
    let plain: Vec<&str> = args.iter().copied().filter(|a| *a != "--recent").collect();
    let state = State::new("empty");
    let with = run(&state, args);
    assert_eq!(with, run(&state, &plain), "{args:?}");
    assert!(!state.path().exists(), "reading created nothing");
}

#[test]
fn recent_ignores_a_history_that_is_not_readable() {
    let state = State::new("garbled");
    let dir = state.path().join("sshpick");
    fs::create_dir_all(dir.join("history")).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(hosts(&state, &["ls", "--recent"]), CONFIG_ORDER);
    fs::remove_dir(dir.join("history")).unwrap_or_else(|error| panic!("{error}"));
    fs::write(dir.join("history"), "not a history\n\u{0}\u{1}\n5\n")
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(hosts(&state, &["ls", "--recent"]), CONFIG_ORDER);
}

#[test]
fn recent_only_reads_the_history() {
    let seeded = State::seeded("read");
    let file = seeded.path().join("sshpick/history");
    let before = fs::read(&file).unwrap_or_default();
    run(&seeded, &["ls", "--recent"]);
    run(&seeded, &["ls", "--recent", "--json", "lab"]);
    assert_eq!(fs::read(&file).unwrap_or_default(), before);
    assert_eq!(
        fs::read_dir(seeded.path().join("sshpick"))
            .map(|entries| entries.count())
            .unwrap_or_default(),
        1,
        "no other file appeared"
    );

    let absent = State::new("read-absent");
    run(&absent, &["ls", "--recent"]);
    assert!(!absent.path().exists());
}
