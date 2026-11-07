mod common;

use std::io;
use std::process::{Output, Stdio};

use assert_cmd::Command;
use common::{isolated, sshpick, sshpick_std, stderr};
use insta::assert_snapshot;
use test_case::test_case;

fn with_stdout_closed(home: &str, args: &[&str]) -> io::Result<Output> {
    let (reader, writer) = io::pipe()?;
    drop(reader);
    sshpick_std(home)
        .args(args)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
}

fn with_stderr_closed(home: &str, args: &[&str]) -> io::Result<Output> {
    let (reader, writer) = io::pipe()?;
    drop(reader);
    sshpick_std(home)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(writer)
        .output()
}

fn sp(home: &str) -> Command {
    Command::from_std(isolated(env!("CARGO_BIN_EXE_sp"), home))
}

#[test]
fn sp_is_the_same_program_under_a_short_name() {
    for args in [
        &["--version"][..],
        &["ls"],
        &["ls", "--json"],
        &["inspect", "stg-consul1"],
    ] {
        let long = sshpick("inspect").args(args).assert().success();
        let short = sp("inspect").args(args).assert().success();
        assert_eq!(
            short.get_output().stdout,
            long.get_output().stdout,
            "sp {args:?} prints what sshpick does"
        );
    }
    sp("inspect").arg("--bogus").assert().code(2).stdout("");
}

#[test]
fn version_and_help_exit_zero() {
    sshpick("home").arg("--version").assert().success();
    sshpick("home").arg("-h").assert().success();
}

#[test]
fn an_unknown_option_exits_two_and_names_what_is_accepted() {
    let out = sshpick("home").arg("--bogus").assert().code(2).stdout("");
    assert_snapshot!("unknown_option", stderr(&out));
    let out = sshpick("home").arg("--lst").assert().code(2).stdout("");
    assert_snapshot!("unknown_option_with_a_suggestion", stderr(&out));
    let out = sshpick("home")
        .args(["list", "--lsit"])
        .assert()
        .code(2)
        .stdout("");
    assert_snapshot!("unknown_option_for_a_command", stderr(&out));
}

#[test]
fn read_only_commands_exit_zero_when_stdout_closes_early() {
    for (home, args) in [
        ("ui", &["--help"][..]),
        ("ui", &["-V"][..]),
        ("ui", &["--list"][..]),
        ("inspect", &["ls", "--json"][..]),
        ("inspect", &["inspect", "stg-consul1", "--json"][..]),
        ("inspect", &["doctor", "--json"][..]),
    ] {
        let out = with_stdout_closed(home, args).unwrap();
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        assert_eq!(String::from_utf8_lossy(&out.stderr), "", "{args:?}");
    }
}

#[test]
fn help_goes_to_stdout_and_leads_with_the_version() {
    let out = sshpick("home").arg("--help").assert().code(0).stderr("");
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    let lead = format!(
        "sshpick {}\n\n  ▌ sshpick [command] [query]... [options]\n",
        env!("CARGO_PKG_VERSION")
    );
    assert!(text.starts_with(&lead));
    assert!(!text.contains('\x1b'), "a pipe gets no escape bytes");
    assert!(text.lines().all(|line| !line.ends_with(' ')));
}

#[test]
fn help_for_a_command_is_the_same_page_three_ways() {
    let page = |args: &[&str]| {
        let out = sshpick("home").args(args).assert().code(0).stderr("");
        String::from_utf8_lossy(&out.get_output().stdout).into_owned()
    };
    let list = page(&["list", "--help"]);
    assert!(list.starts_with("\n  ▌ sshpick list [query]... [options]\n"));
    assert_eq!(list, page(&["ls", "-h"]));
    assert_eq!(list, page(&["help", "ls"]));
    assert_eq!(list, page(&["help", "list"]));
}

#[test]
fn help_for_doctor_is_the_same_page_three_ways() {
    let page = |args: &[&str]| {
        let out = sshpick("home").args(args).assert().code(0).stderr("");
        String::from_utf8_lossy(&out.get_output().stdout).into_owned()
    };
    let doctor = page(&["doctor", "--help"]);
    assert!(doctor.starts_with("\n  ▌ sshpick doctor [options]\n"));
    assert_eq!(doctor, page(&["doctor", "-h"]));
    assert_eq!(doctor, page(&["help", "doctor"]));
}

#[test_case(&["--version"]; "version")]
#[test_case(&["--help"]; "help")]
#[test_case(&["help", "doctor"]; "help for a command")]
#[test_case(&["ls", "--json"]; "ls as json")]
#[test_case(&["inspect", "stg-consul1", "--json"]; "inspect as json")]
#[test_case(&["doctor", "--json"]; "doctor as json")]
fn a_read_only_command_leaves_stderr_empty(args: &[&str]) {
    let out = sshpick("inspect").args(args).assert().code(0);
    assert_eq!(stderr(&out), "", "{args:?}");
}

#[test_case("inspect", &["inspect", "zzz", "--json"], 1, "no_match", "action_required", true; "no match")]
#[test_case("inspect", &["inspect", "consul", "--json"], 2, "several_matches", "action_required", true; "several matches")]
#[test_case("inspect", &["inspect", "stg-broken", "--json"], 2, "ssh_failed", "error", false; "ssh -G failed")]
#[test_case("slow", &["inspect", "stg-slow", "--json"], 2, "timeout", "error", true; "ssh -G timed out")]
#[test_case("empty", &["ls", "--json"], 2, "no_hosts", "error", true; "no hosts")]
#[test_case("home", &["pick", "--json"], 2, "usage", "action_required", true; "pick has no json")]
#[test_case("home", &["help", "nosuch", "--json"], 2, "usage", "action_required", true; "help for an unknown command")]
fn a_failure_under_json_is_one_line_on_stderr(
    home: &str,
    args: &[&str],
    code: i32,
    reason: &str,
    status: &str,
    has_fix: bool,
) -> Result<(), serde_json::Error> {
    let out = sshpick(home).args(args).assert().code(code).stdout("");
    let text = stderr(&out);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.ends_with('\n'));
    let document: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(document["reason"], reason);
    assert_eq!(document["status"], status);
    assert!(document["message"].is_string());
    assert_eq!(document.get("fix").is_some(), has_fix, "{text}");
    Ok(())
}

#[test]
fn a_config_error_under_json_is_one_line_on_stderr() -> Result<(), serde_json::Error> {
    let out = sshpick("home")
        .env("HOME", "/nonexistent-home")
        .args(["ls", "--json"])
        .assert()
        .code(2)
        .stdout("");
    let text = stderr(&out);
    assert_eq!(text.lines().count(), 1, "{text}");
    let document: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(document["reason"], "config_error");
    assert_eq!(document["status"], "error");
    Ok(())
}

#[test]
fn a_misspelt_json_flag_was_not_a_request_for_json() {
    let out = sshpick("home")
        .args(["ls", "--jsn"])
        .assert()
        .code(2)
        .stdout("");
    assert!(stderr(&out).starts_with("Error: unknown option \"--jsn\"\n"));
}

#[test]
fn a_failure_under_json_carries_the_warnings_the_run_found() {
    let out = sshpick("cycle")
        .args(["inspect", "zzz", "--json"])
        .assert()
        .code(1)
        .stdout("");
    let document: serde_json::Value = serde_json::from_str(&stderr(&out)).unwrap();
    assert_eq!(
        document["warnings"],
        serde_json::json!([{
            "kind": "cycle",
            "path": "<fixtures>/cycle/.ssh/config",
            "message": "<fixtures>/cycle/.ssh/config: skipped, include cycle",
        }])
    );
}

#[test]
fn several_matches_under_json_lists_every_candidate_best_first() {
    let out = sshpick("inspect")
        .args(["inspect", "consul", "--json"])
        .assert()
        .code(2)
        .stdout("");
    let document: serde_json::Value = serde_json::from_str(&stderr(&out)).unwrap();
    assert_eq!(
        document["candidates"],
        serde_json::json!(["stg-consul1", "stg-consul2"])
    );
    let out = sshpick("inspect")
        .args(["inspect", "zzz", "--json"])
        .assert()
        .code(1)
        .stdout("");
    let document: serde_json::Value = serde_json::from_str(&stderr(&out)).unwrap();
    assert!(document.get("candidates").is_none(), "{document}");
}

#[test]
fn help_under_json_is_still_help() {
    let out = sshpick("home")
        .args(["--help", "--json"])
        .assert()
        .code(0)
        .stderr("");
    assert!(String::from_utf8_lossy(&out.get_output().stdout).starts_with("sshpick "));
}

#[test]
fn help_wins_over_an_unknown_option() {
    sshpick("home")
        .args(["--bogus", "-h"])
        .assert()
        .code(0)
        .stderr("");
}

#[test]
fn help_for_an_unknown_command_exits_two_and_suggests_one_edit() {
    let out = sshpick("home")
        .args(["help", "lst"])
        .assert()
        .code(2)
        .stdout("");
    assert_snapshot!("unknown_command", stderr(&out));
}

#[test]
fn version_prints_one_line_and_a_hidden_lowercase_alias_agrees() {
    for flag in ["-V", "--version", "-v"] {
        sshpick("home")
            .arg(flag)
            .assert()
            .code(0)
            .stdout(format!("sshpick {}\n", env!("CARGO_PKG_VERSION")))
            .stderr("");
    }
}

#[test]
fn version_works_after_any_command_without_running_ssh() {
    for command in ["pick", "doctor", "inspect", "ls"] {
        for flag in ["-V", "--version"] {
            sshpick("home")
                .args([command, flag])
                .assert()
                .code(0)
                .stdout(format!("sshpick {}\n", env!("CARGO_PKG_VERSION")))
                .stderr("");
        }
    }
}

#[test]
fn colour_follows_the_environment_not_just_the_terminal() {
    let escapes = |vars: &[(&str, &str)]| {
        let mut command = sshpick("home");
        command.arg("--help");
        for (name, value) in vars {
            command.env(name, value);
        }
        let out = command.assert().code(0);
        String::from_utf8_lossy(&out.get_output().stdout).contains('\x1b')
    };
    assert!(!escapes(&[]), "a pipe is plain");
    assert!(escapes(&[("CLICOLOR_FORCE", "1")]), "forced colour");
    assert!(!escapes(&[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "1")]));
    assert!(!escapes(&[("CLICOLOR_FORCE", "0")]));
    assert!(!escapes(&[
        ("CLICOLOR_FORCE", "1"),
        ("TERM", "dumb"),
        ("NO_COLOR", "yes")
    ]));
}

#[test]
fn a_usage_error_honours_no_color_until_a_double_dash() {
    let escapes = |args: &[&str]| {
        let out = sshpick("home")
            .args(args)
            .env("CLICOLOR_FORCE", "1")
            .assert()
            .code(2)
            .stdout("");
        stderr(&out).contains('\x1b')
    };
    assert!(escapes(&["--bogus"]), "forced colour reaches the error");
    assert!(!escapes(&["--bogus", "--no-color"]));
    assert!(!escapes(&["--no-color", "--bogus"]));
    assert!(escapes(&["--bogus", "--", "--no-color"]));
}

#[test]
fn no_color_removes_the_hue_even_when_colour_is_forced() {
    let out = sshpick("home")
        .args(["--no-color", "--help"])
        .env("CLICOLOR_FORCE", "1")
        .assert()
        .code(0);
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(
        !text.contains("\x1b[36m") && !text.contains("\x1b[90m"),
        "no hue"
    );
}

#[test]
fn a_closed_stderr_keeps_the_commands_own_exit_code() {
    let out = with_stderr_closed("home", &["--bogus"]).unwrap();
    assert_eq!(out.status.code(), Some(2));
    let out = with_stderr_closed("warns", &["ls"]).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "warnings on a closed stderr are dropped"
    );
    assert!(!out.stdout.is_empty(), "the rows still reach stdout");
}

#[test]
fn argv_with_control_characters_is_printed_as_spaces() {
    let out = sshpick("home")
        .args(["\u{1b}]0;x\u{7} zz", "--non-interactive"])
        .assert()
        .code(1)
        .stdout("");
    let text = stderr(&out);
    assert!(!text.contains(['\u{1b}', '\u{7}']), "{text:?}");
    assert!(
        text.starts_with("Error: no host matches \" ]0;x  zz\"\n"),
        "{text:?}"
    );
}

#[test]
fn a_fix_quotes_a_query_that_the_shell_would_misread() {
    let out = sshpick("inspect")
        .args(["inspect", "'stg", "--non-interactive"])
        .assert()
        .code(2)
        .stdout("");
    assert!(
        stderr(&out).contains("`sshpick ls ''\\''stg'`"),
        "{}",
        stderr(&out)
    );
}
