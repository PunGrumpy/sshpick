mod common;

use common::{sshpick, stderr};
use insta::assert_snapshot;
use serde_json::json;

const TEXT: &str = "  General\n\n    host   stg-consul1\n    env    STAGING\n    what   consul server 1\n    names  stg-consul1, consul1.stg\n\n  Resolved\n\n    user       ubuntu\n    hostname   192.0.2.11\n    port       22\n    proxyjump  stg-bastion\n\n  Command\n\n    ssh -- stg-consul1\n";

#[test]
fn inspect_prints_the_host_what_ssh_says_and_the_command() {
    let out = sshpick("inspect")
        .args(["inspect", "stg-consul1"])
        .assert()
        .code(0)
        .stdout(TEXT);
    assert_eq!(stderr(&out), "", "the > line is for a terminal");
}

#[test]
fn inspect_takes_a_query_and_finds_a_host_by_its_alias() {
    sshpick("inspect")
        .args(["inspect", "consul1.stg"])
        .assert()
        .code(0)
        .stdout(TEXT);
    sshpick("inspect")
        .args(["inspect", "consul", "1"])
        .assert()
        .code(0)
        .stdout(TEXT);
}

#[test]
fn inspect_json_has_the_host_what_ssh_resolved_and_the_command() {
    let out = sshpick("inspect")
        .args(["inspect", "stg-consul1", "--json"])
        .assert()
        .code(0);
    assert_eq!(stderr(&out), "");
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.starts_with("{\n  \"host\": \"stg-consul1\",\n  \"env\": \"STAGING\","));
    assert!(text.ends_with("}\n"), "one trailing newline");
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        value,
        json!({
            "host": "stg-consul1",
            "env": "STAGING",
            "what": "consul server 1",
            "names": ["stg-consul1", "consul1.stg"],
            "resolved": {
                "user": "ubuntu",
                "hostname": "192.0.2.11",
                "port": "22",
                "proxyjump": "stg-bastion",
            },
            "command": ["ssh", "--", "stg-consul1"],
        })
    );
}

#[test]
fn inspect_json_keeps_empty_strings_and_leaves_out_keys_ssh_did_not_print() {
    let out = sshpick("inspect")
        .args(["--json", "inspect", "list"])
        .assert()
        .code(0);
    let value: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(value["env"], "");
    assert_eq!(value["what"], "the host that is also a command");
    assert!(value["resolved"].get("remotecommand").is_none());
}

#[test]
fn several_matches_exit_two_and_name_the_candidates() {
    let out = sshpick("inspect")
        .args(["inspect", "consul"])
        .assert()
        .code(2)
        .stdout("");
    assert_snapshot!("several", stderr(&out));
    sshpick("inspect")
        .args(["inspect", "consul", "--json"])
        .assert()
        .code(2)
        .stdout("");
}

#[test]
fn no_match_exits_one_and_a_typo_in_the_command_word_is_spotted() {
    let out = sshpick("inspect")
        .args(["inspect", "zzz"])
        .assert()
        .code(1)
        .stdout("");
    assert_snapshot!("no_match", stderr(&out));
    let out = sshpick("inspect")
        .args(["inspekt", "stg-consul1", "zzz"])
        .assert()
        .code(1)
        .stdout("");
    assert_snapshot!("typo", stderr(&out));
}

#[test]
fn a_failing_ssh_g_exits_two_with_nothing_on_stdout() {
    for args in [
        &["inspect", "stg-broken"][..],
        &["inspect", "stg-broken", "--json"][..],
    ] {
        let out = sshpick("inspect").args(args).assert().code(2).stdout("");
        assert_snapshot!("ssh_g_failed", stderr(&out));
    }
}

#[test]
fn a_missing_ssh_says_how_to_fix_it() {
    let out = sshpick("inspect")
        .env("PATH", "/nonexistent-path")
        .args(["inspect", "stg-consul1"])
        .assert()
        .code(2)
        .stdout("");
    assert_snapshot!("ssh_missing", stderr(&out));
}

#[test]
fn inspect_needs_a_query() {
    let out = sshpick("inspect")
        .arg("inspect")
        .assert()
        .code(2)
        .stdout("");
    assert_snapshot!("missing_query", stderr(&out));
}

#[test]
fn inspect_is_a_dry_run_of_pick() {
    let command = sshpick("inspect")
        .args(["--json", "inspect", "stg-consul2"])
        .assert()
        .code(0);
    let value: serde_json::Value = serde_json::from_slice(&command.get_output().stdout).unwrap();
    let argv: Vec<&str> = value["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|part| part.as_str().unwrap())
        .collect();
    assert_eq!(argv, ["ssh", "--", "stg-consul2"]);
    sshpick("inspect")
        .arg("stg-consul2")
        .assert()
        .code(0)
        .stdout("ssh -- stg-consul2\n");
}
