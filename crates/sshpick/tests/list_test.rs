mod common;

use common::{sshpick, stderr};
use insta::assert_snapshot;
use serde_json::json;

#[test]
fn list_prints_every_host_as_tsv_in_file_order() {
    let out = sshpick("ui").arg("--list").assert().code(0).stdout(
        "LAB\tlab-a\tLAB — personal VMs\n\
         LAB\tlab-b\tbuild box\n\
         NEW STAGING\tnewstg-consul1\tconsul server 1 — mgmt\n\
         NEW STAGING\tnewstg-consul2\t\n\
         NEW STAGING\tnewstg-bastion\tbastion\n",
    );
    assert_eq!(stderr(&out), "");
}

#[test]
fn list_filters_with_all_words_and_ranks_best_first() {
    sshpick("ui")
        .args(["-l", "new", "consul"])
        .assert()
        .code(0)
        .stdout(
            "NEW STAGING\tnewstg-consul1\tconsul server 1 — mgmt\n\
             NEW STAGING\tnewstg-consul2\t\n",
        );
}

#[test]
fn list_keeps_three_columns_when_env_is_empty() {
    sshpick("flat")
        .arg("--list")
        .assert()
        .stdout("\troot-a\treachable from anywhere\n\troot-b\t\n");
    sshpick("tabs")
        .arg("--list")
        .assert()
        .stdout("\ttabbed\tone two\n");
}

#[test]
fn list_with_no_match_prints_nothing_and_exits_one() {
    sshpick("ui")
        .args(["--list", "zzz"])
        .assert()
        .code(1)
        .stdout("")
        .stderr("");
}

const UI_ROWS: &str = "LAB\tlab-a\tLAB — personal VMs\n\
                       LAB\tlab-b\tbuild box\n\
                       NEW STAGING\tnewstg-consul1\tconsul server 1 — mgmt\n\
                       NEW STAGING\tnewstg-consul2\t\n\
                       NEW STAGING\tnewstg-bastion\tbastion\n";

#[test]
fn list_and_ls_print_the_same_tsv_as_dash_dash_list_on_a_pipe() {
    for command in ["list", "ls"] {
        sshpick("ui")
            .arg(command)
            .assert()
            .code(0)
            .stdout(UI_ROWS)
            .stderr("");
    }
    sshpick("ui").arg("-l").assert().stdout(UI_ROWS);
}

#[test]
fn ls_filters_and_ranks_like_the_picker() {
    sshpick("ui")
        .args(["ls", "new", "consul"])
        .assert()
        .code(0)
        .stdout(
            "NEW STAGING\tnewstg-consul1\tconsul server 1 — mgmt\n\
             NEW STAGING\tnewstg-consul2\t\n",
        );
}

#[test]
fn ls_with_no_match_prints_nothing_and_exits_one() {
    sshpick("ui")
        .args(["ls", "zzz"])
        .assert()
        .code(1)
        .stdout("")
        .stderr("");
}

#[test]
fn a_command_word_lists_instead_of_searching_for_the_word() {
    sshpick("words")
        .arg("list")
        .assert()
        .code(0)
        .stdout("\tlist\t\n\tweb\t\n");
}

#[test]
fn list_json_prints_the_query_the_total_and_every_match() {
    let out = sshpick("inspect")
        .args(["list", "consul", "--json"])
        .assert()
        .code(0);
    assert_eq!(stderr(&out), "");
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.starts_with("{\n  \"query\": \"consul\",\n  \"total\": 4,"));
    assert!(text.ends_with("}\n"));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        value,
        json!({
            "query": "consul",
            "total": 4,
            "hosts": [
                {
                    "host": "stg-consul1",
                    "env": "STAGING",
                    "what": "consul server 1",
                    "names": ["stg-consul1", "consul1.stg"],
                },
                {
                    "host": "stg-consul2",
                    "env": "STAGING",
                    "what": "consul server 2",
                    "names": ["stg-consul2"],
                },
            ],
            "warnings": [],
        })
    );
}

#[test]
fn json_beats_dash_dash_list() {
    let json = sshpick("inspect").args(["list", "--json"]).assert().code(0);
    let both = sshpick("inspect")
        .args(["list", "--list", "--json"])
        .assert()
        .code(0)
        .stderr("");
    assert_eq!(
        both.get_output().stdout,
        json.get_output().stdout,
        "--list adds nothing to --json"
    );
    let text = String::from_utf8_lossy(&both.get_output().stdout).into_owned();
    assert!(text.starts_with("{\n  \"query\": \"\","), "{text}");
}

#[test]
fn list_json_has_the_same_order_as_the_rows() {
    let rows = sshpick("ui").arg("ls").assert().code(0);
    let hosts: Vec<String> = String::from_utf8_lossy(&rows.get_output().stdout)
        .lines()
        .map(|line| line.split('\t').nth(1).unwrap_or_default().to_owned())
        .collect();
    let out = sshpick("ui").args(["ls", "--json"]).assert().code(0);
    let value: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let listed: Vec<&str> = value["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|host| host["host"].as_str().unwrap())
        .collect();
    assert_eq!(listed, hosts);
}

#[test]
fn list_json_with_no_match_prints_the_object_and_exits_one() {
    let out = sshpick("inspect")
        .args(["ls", "zzz", "--json"])
        .assert()
        .code(1);
    assert_eq!(stderr(&out), "");
    let value: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(
        value,
        json!({"query": "zzz", "total": 4, "hosts": [], "warnings": []})
    );
}

#[test]
fn list_json_carries_the_warnings_the_run_found() {
    let out = sshpick("cycle").args(["ls", "--json"]).assert().code(0);
    let path = common::fixtures().join("cycle/.ssh/config");
    let value: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(
        value["warnings"],
        json!([{
            "kind": "cycle",
            "path": path.display().to_string(),
            "message": format!("{}: skipped, include cycle", path.display()),
        }])
    );
    assert_snapshot!("warning_still_on_stderr", stderr(&out));
}
