mod common;

use common::{sshpick, stderr};
use insta::assert_snapshot;

#[test]
fn one_match_execs_ssh_with_double_dash_and_no_tty() {
    sshpick("home")
        .assert()
        .success()
        .stdout("ssh -- only-host\n");
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
