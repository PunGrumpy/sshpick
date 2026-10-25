mod common;

use assert_cmd::assert::Assert;
use common::{fixtures, sshpick, stderr};
use insta::assert_snapshot;
use serde_json::Value;
use test_case::test_case;

fn stdout(assert: &Assert) -> String {
    let text = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    without_timing(&text.replace(&fixtures().display().to_string(), "<fixtures>"))
}

fn without_timing(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(" in ") {
        let (head, tail) = rest.split_at(at + " in ".len());
        out.push_str(head);
        let digits = tail.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 && tail[digits..].starts_with(" ms") {
            out.push_str("<n>");
            rest = &tail[digits..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn report(home: &str, code: i32) -> String {
    let out = sshpick(home).arg("doctor").assert().code(code);
    assert_eq!(stderr(&out), "", "a failed check is still only a report");
    stdout(&out)
}

fn report_as_json(home: &str, code: i32) -> Value {
    let out = sshpick(home).args(["doctor", "--json"]).assert().code(code);
    assert_eq!(stderr(&out), "");
    let text = stdout(&out);
    assert!(text.ends_with("}\n"), "one trailing newline");
    match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => panic!("not JSON: {error}\n{text}"),
    }
}

fn word(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

#[test]
fn a_sound_setup_exits_zero_and_warns_that_a_pipe_cannot_open_the_picker() {
    assert_snapshot!("doctor_ui", report("ui", 0));
}

#[test]
fn a_config_without_hosts_fails_the_report_and_exits_two() {
    assert_snapshot!("doctor_empty", report("empty", 2));
}

#[test]
fn a_skipped_include_is_a_warning_and_not_a_failure() {
    let text = report("cycle", 0);
    assert_snapshot!("doctor_cycle", text);
}

#[test]
fn a_failing_ssh_g_for_the_first_host_fails_resolve() {
    assert_snapshot!("doctor_broken", report("broken", 2));
}

#[test]
fn a_missing_ssh_fails_the_ssh_check_and_leaves_resolve_out() {
    let out = sshpick("ui")
        .arg("doctor")
        .env("PATH", "/nonexistent-path")
        .assert()
        .code(2);
    assert_eq!(stderr(&out), "");
    assert_snapshot!("doctor_no_ssh", stdout(&out));
}

#[test]
fn an_unreadable_config_fails_the_config_check_and_leaves_hosts_out() {
    let out = sshpick("ui")
        .arg("doctor")
        .env("HOME", "/nonexistent-home")
        .assert()
        .code(2);
    assert_eq!(stderr(&out), "");
    assert_snapshot!("doctor_no_config", stdout(&out));
}

#[test]
fn json_is_the_pretty_report_with_a_fix_only_where_there_is_one() {
    let out = sshpick("empty").args(["doctor", "--json"]).assert().code(2);
    assert_eq!(stderr(&out), "");
    let text = stdout(&out);
    assert!(text.starts_with(
        "{\n  \"config\": \"<fixtures>/empty/.ssh/config\",\n  \"summary\": {\n    \"ok\": 2,"
    ));
    assert_snapshot!("doctor_empty_json", text);
}

#[test_case("ui", 0; "a sound setup")]
#[test_case("empty", 2; "no hosts")]
#[test_case("cycle", 0; "a skipped include")]
#[test_case("broken", 2; "ssh -G fails")]
fn the_text_and_the_json_are_one_report(home: &str, code: i32) {
    let json = report_as_json(home, code);
    let text = report(home, code);
    let mut lines = text.lines();
    let count = |status: &str| json["summary"][status].as_u64().unwrap_or_default();
    assert_eq!(
        lines.next(),
        Some(
            format!(
                "[doctor] ok={} warn={} fail={}",
                count("ok"),
                count("warn"),
                count("fail")
            )
            .as_str()
        )
    );
    assert_eq!(
        lines.next(),
        Some(format!("[doctor] config={}", word(&json["config"])).as_str())
    );
    let checks = json["checks"].as_array().cloned().unwrap_or_default();
    let want: Vec<String> = checks
        .iter()
        .map(|check| {
            let fix = check["fix"]
                .as_str()
                .map_or_else(String::new, |fix| format!("; {fix}"));
            format!(
                "[{}] {}: {}{fix}",
                word(&check["status"]),
                word(&check["name"]),
                word(&check["detail"])
            )
        })
        .collect();
    assert_eq!(lines.collect::<Vec<_>>(), want);
}

fn documented_names() -> Vec<String> {
    let docs = include_str!("../../../docs/scripting.md");
    docs.lines()
        .skip_while(|line| !line.starts_with("### `doctor --json`"))
        .skip_while(|line| !line.starts_with("| Check"))
        .skip(2)
        .take_while(|line| line.starts_with('|'))
        .filter_map(|line| line.split('`').nth(1))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_check_names_are_exactly_the_documented_set() {
    let documented = documented_names();
    assert_eq!(
        documented,
        ["config", "hosts", "include", "ssh", "resolve", "terminal"]
    );
    let mut seen: Vec<String> = Vec::new();
    for (home, code) in [("ui", 0), ("cycle", 0), ("empty", 2), ("broken", 2)] {
        let json = report_as_json(home, code);
        let names: Vec<String> = json["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|check| check["name"].as_str().unwrap().to_owned())
            .collect();
        let at = |name: &String| documented.iter().position(|doc| doc == name);
        assert!(
            names.iter().all(|name| at(name).is_some()),
            "{home}: {names:?} has a name that is not documented"
        );
        assert!(
            names.windows(2).all(|pair| at(&pair[0]) <= at(&pair[1])),
            "{home}: {names:?} is not in the documented order"
        );
        for name in names {
            if !seen.contains(&name) {
                seen.push(name);
            }
        }
    }
    seen.sort_by_key(|name| documented.iter().position(|doc| doc == name));
    assert_eq!(seen, documented, "these runs reach every documented check");
}

#[test]
fn colour_reaches_only_the_bracketed_token_and_only_when_it_is_on() {
    let run = |env: &[(&str, &str)]| {
        let mut command = sshpick("empty");
        command.arg("doctor");
        for (name, value) in env {
            command.env(name, value);
        }
        stdout(&command.assert().code(2))
    };
    let text = run(&[("CLICOLOR_FORCE", "1")]);
    assert!(text.contains("[ok] config: read"), "ok is plain: {text}");
    assert!(
        text.contains("\x1b[31m[fail]\x1b[39m hosts: no hosts found"),
        "{text}"
    );
    assert!(
        text.contains(
            "\x1b[33m[warn]\x1b[39m terminal: TERM is not set, colour on (CLICOLOR_FORCE is set);"
        ),
        "{text}"
    );
    assert_eq!(
        text.matches('\x1b').count(),
        4,
        "two tokens, an opening and a closing escape each: {text}"
    );
    let plain = run(&[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "1")]);
    assert!(!plain.contains('\x1b'), "{plain}");
    assert!(plain.contains("colour off (NO_COLOR is set)"), "{plain}");
    assert!(!run(&[]).contains('\x1b'), "a pipe is plain");
}

#[test]
fn the_terminal_check_explains_the_refusal_pick_would_make() {
    let terminal = |args: &[&str], env: &[(&str, &str)]| {
        let mut command = sshpick("ui");
        command.args(args).arg("doctor");
        for (name, value) in env {
            command.env(name, value);
        }
        let text = stdout(&command.assert().code(0));
        text.lines()
            .find(|line| line.starts_with("[warn] terminal:"))
            .unwrap_or_default()
            .to_owned()
    };
    assert!(
        terminal(&[], &[]).contains(
            "TERM is not set, colour off (stdout is not a terminal); the picker needs an interactive terminal; `sshpick <query>`"
        )
    );
    assert!(
        terminal(&["--non-interactive"], &[]).contains(
            "colour off (stdout is not a terminal); --non-interactive never opens the picker; leave out `--non-interactive`"
        )
    );
    assert!(terminal(&[], &[("TERM", "dumb")]).contains("TERM=dumb, colour off (TERM=dumb);"));
}

#[test]
fn doctor_takes_no_arguments() {
    let out = sshpick("ui")
        .args(["doctor", "extra"])
        .assert()
        .code(2)
        .stdout("");
    assert_snapshot!("doctor_extra_argument", stderr(&out));
}

#[test]
fn a_host_named_doctor_is_reached_with_pick_or_double_dash_and_doctor_still_checks() {
    for args in [&["pick", "doctor"][..], &["--", "doctor"][..]] {
        sshpick("named-doctor")
            .args(args)
            .assert()
            .code(0)
            .stdout("ssh -- doctor\n")
            .stderr("");
    }
    let out = sshpick("named-doctor").arg("doctor").assert().code(0);
    assert!(stdout(&out).starts_with("[doctor] ok=4 warn=1 fail=0\n"));
    assert_eq!(
        stderr(&out),
        "",
        "the note about a host named doctor is for a terminal"
    );
}
