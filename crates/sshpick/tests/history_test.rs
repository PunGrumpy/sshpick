//! Without a terminal sshpick neither reads nor writes the history, so what a script sees is a
//! function of the config and the query. The picker's half is in `picker_test.rs`, on a pty.

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
        let state = State::new(name);
        let dir = state.0.join("sshpick");
        fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("cannot create {dir:?}: {error}"));
        let lines = "1794556700\tnewstg-consul2\n1794556790\tlab-b\n";
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
