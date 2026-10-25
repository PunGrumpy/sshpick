mod common;

use common::{sshpick, stderr};
use insta::assert_snapshot;
use test_case::test_case;

#[test]
fn one_match_execs_ssh_with_double_dash_and_no_tty() {
    sshpick("home")
        .assert()
        .success()
        .stdout("ssh -- only-host\n");
}

#[test]
fn the_one_match_path_through_a_pipe_says_nothing_on_stderr() {
    sshpick("home")
        .assert()
        .success()
        .stdout("ssh -- only-host\n")
        .stderr("");
    sshpick("ui")
        .args(["--non-interactive", "lab-a"])
        .assert()
        .success()
        .stdout("ssh -- lab-a\n")
        .stderr("");
}

#[test]
fn several_matches_without_a_terminal_exit_two_and_leave_stdout_alone() {
    let out = sshpick("ui").assert().code(2).stdout("");
    assert_snapshot!("no_terminal_several", stderr(&out));
    let out = sshpick("ui").arg("consul").assert().code(2).stdout("");
    assert_snapshot!("no_terminal_consul", stderr(&out));
}

#[test]
fn zero_matches_without_a_terminal_exit_one() {
    let out = sshpick("ui").arg("zzz").assert().code(1).stdout("");
    assert_snapshot!("no_terminal_zero", stderr(&out));
}

#[test]
fn a_misspelt_command_is_a_query_that_finds_nothing_and_says_what_it_looks_like() {
    let out = sshpick("ui")
        .args(["lst", "zzz"])
        .assert()
        .code(1)
        .stdout("");
    assert_snapshot!("no_terminal_lst", stderr(&out));
}

#[test]
fn non_interactive_never_opens_the_picker() {
    sshpick("home")
        .arg("--non-interactive")
        .assert()
        .code(0)
        .stdout("ssh -- only-host\n");
    let out = sshpick("ui")
        .arg("--non-interactive")
        .assert()
        .code(2)
        .stdout("");
    assert_snapshot!("non_interactive_several", stderr(&out));
    let out = sshpick("ui")
        .args(["--non-interactive", "zzz"])
        .assert()
        .code(1)
        .stdout("");
    assert_snapshot!("non_interactive_zero", stderr(&out));
}

#[test]
fn a_config_without_hosts_exits_two_for_the_picker_and_for_list() {
    for args in [&[][..], &["--list"][..], &["ls"][..]] {
        let out = sshpick("empty").args(args).assert().code(2).stdout("");
        assert_snapshot!("no_hosts", stderr(&out));
    }
}

#[test]
fn unreadable_config_exits_two() {
    let out = sshpick("home")
        .env("HOME", "/nonexistent-home")
        .assert()
        .code(2)
        .stdout("");
    assert_snapshot!("unreadable_config", stderr(&out));
}

#[test]
fn a_missing_ssh_exits_127_and_says_how_to_fix_it() {
    let out = sshpick("home")
        .env("PATH", "/nonexistent-path")
        .assert()
        .code(127)
        .stdout("");
    assert_snapshot!("ssh_missing", stderr(&out));
}

#[test]
fn a_warning_is_a_bang_line_and_the_run_goes_on() {
    let out = sshpick("cycle").assert().code(0).stdout("ssh -- solo\n");
    assert_snapshot!("warning", stderr(&out));
}

#[test_case(&[], 2; "several matches")]
#[test_case(&["zzz"], 1; "no match")]
#[test_case(&["warm-a"], 0; "one match")]
#[test_case(&["--non-interactive"], 2; "refused")]
fn a_warning_is_printed_once_and_before_everything_else(args: &[&str], code: i32) {
    let out = sshpick("warns").args(args).assert().code(code);
    let text = stderr(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some("! <fixtures>/warns/.ssh/config: skipped, include cycle"),
        "{text}"
    );
    assert_eq!(text.matches("include cycle").count(), 1, "{text}");
}

#[test]
fn a_host_named_like_a_command_is_reached_with_pick_or_double_dash() {
    for args in [&["pick", "list"][..], &["--", "list"][..]] {
        sshpick("words")
            .args(args)
            .assert()
            .code(0)
            .stdout("ssh -- list\n");
    }
}

#[test]
fn a_command_word_later_in_the_query_is_part_of_the_query() {
    let out = sshpick("words")
        .args(["web", "list"])
        .assert()
        .code(1)
        .stdout("");
    assert_snapshot!("query_with_a_command_word", stderr(&out));
}

#[test]
fn no_message_carries_the_old_prefix() {
    let runs: [(&str, &[&str]); 5] = [
        ("ui", &[]),
        ("ui", &["zzz"]),
        ("empty", &[]),
        ("cycle", &[]),
        ("ui", &["--bogus"]),
    ];
    for (home, args) in runs {
        let out = sshpick(home).args(args).assert();
        let text = stderr(&out);
        assert!(!text.contains("sshpick:"), "{text}");
        assert!(!text.contains("warning:"), "{text}");
    }
}

#[test]
fn a_dumb_terminal_still_connects_when_one_host_matches() {
    sshpick("home")
        .env("TERM", "dumb")
        .assert()
        .success()
        .stdout("ssh -- only-host\n");
}
