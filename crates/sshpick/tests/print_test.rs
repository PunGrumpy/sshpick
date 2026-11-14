//! `pick --print` through a pipe. The fake `ssh` prints how it was called on stdout, so a stdout
//! that holds only the host proves ssh never ran.

mod common;

use common::{sshpick, stderr};
use insta::assert_snapshot;
use test_case::test_case;

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(bytes)))
}

#[test_case(&["--print", "--non-interactive", "--", "stg-consul1"], "stg-consul1\n"; "an exact name")]
#[test_case(&["--print", "--non-interactive", "--", "consul1.stg"], "stg-consul1\n"; "an alias prints the first name")]
#[test_case(&["pick", "--print", "--non-interactive", "stg-consul1"], "stg-consul1\n"; "with the pick word")]
#[test_case(&["--non-interactive", "stg-consul1", "--print"], "stg-consul1\n"; "the option after the query")]
fn print_writes_the_host_and_never_runs_ssh(args: &[&str], want: &str) {
    sshpick("inspect")
        .args(args)
        .assert()
        .success()
        .stdout(want.to_owned())
        .stderr("");
}

#[test]
fn print_json_is_one_host_of_ls_json() {
    let out = sshpick("inspect")
        .args([
            "--print",
            "--json",
            "--non-interactive",
            "--",
            "consul1.stg",
        ])
        .assert()
        .success()
        .stderr("");
    let printed = json(&out.get_output().stdout);
    assert_eq!(
        printed,
        serde_json::json!({
            "host": "stg-consul1",
            "env": "STAGING",
            "what": "consul server 1",
            "names": ["stg-consul1", "consul1.stg"],
        })
    );
    let listed = sshpick("inspect")
        .args(["ls", "--json", "stg-consul1"])
        .assert()
        .success();
    assert_eq!(json(&listed.get_output().stdout)["hosts"][0], printed);
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.ends_with("}\n"), "{text:?}");
}

#[test]
fn print_json_leaves_the_warnings_out_and_stderr_empty() {
    sshpick("warns")
        .args(["--print", "--json", "--non-interactive", "warm-a"])
        .assert()
        .success()
        .stderr("");
    let out = sshpick("warns")
        .args(["--print", "--non-interactive", "warm-a"])
        .assert()
        .success()
        .stdout("warm-a\n");
    assert!(stderr(&out).starts_with("! "), "{}", stderr(&out));
}

#[test]
fn a_loose_match_prints_when_only_the_terminal_is_missing() {
    sshpick("names")
        .args(["--print", "db-pr"])
        .assert()
        .success()
        .stdout("db-prod\n");
}

// Exit codes and reasons are pick's own, so a script reads them the same way.
#[test_case(&["--non-interactive", "db-pr"], 1, "no_match"; "a loose match under non-interactive")]
#[test_case(&["--non-interactive", "zzz"], 1, "no_match"; "no match")]
#[test_case(&["zzz"], 1, "no_match"; "no match without a terminal")]
#[test_case(&["--non-interactive", "web"], 0, ""; "an exact name that prefixes another")]
#[test_case(&["--non-interactive"], 2, "several_matches"; "several under non-interactive")]
#[test_case(&[], 2, "several_matches"; "several without a terminal")]
fn print_json_exits_as_pick_does(args: &[&str], code: i32, reason: &str) {
    let out = sshpick("names")
        .args(["--print", "--json"])
        .args(args)
        .assert()
        .code(code);
    let output = out.get_output();
    if code == 0 {
        assert!(output.stderr.is_empty());
        return;
    }
    assert!(output.stdout.is_empty(), "stdout stays empty on failure");
    let text = String::from_utf8_lossy(&output.stderr);
    assert_eq!(text.lines().count(), 1, "{text}");
    let document = json(&output.stderr);
    assert_eq!(document["reason"], reason, "{text}");
    if reason == "several_matches" {
        assert_eq!(
            document["candidates"],
            serde_json::json!(["web", "web-2", "db-prod"])
        );
    }
}

#[test]
fn print_on_a_config_error_is_the_json_line_rex_reads() {
    let out = sshpick("empty")
        .args(["--print", "--json", "--non-interactive", "x"])
        .assert()
        .code(2)
        .stdout("");
    let document = json(&out.get_output().stderr);
    assert_eq!(document["reason"], "no_hosts");
    assert!(document["fix"].is_string());
}

#[test]
fn json_on_pick_without_print_is_a_usage_error() {
    for args in [&["--json"][..], &["pick", "--json", "web"][..]] {
        let out = sshpick("names").args(args).assert().code(2).stdout("");
        let document = json(&out.get_output().stderr);
        assert_eq!(document["reason"], "usage");
        assert_eq!(
            document["message"],
            "sshpick pick takes --json only with --print"
        );
    }
}

#[test]
fn print_is_a_usage_error_on_other_commands() {
    let out = sshpick("names")
        .args(["ls", "--print"])
        .assert()
        .code(2)
        .stdout("");
    assert_snapshot!("print_on_ls", stderr(&out));
}
