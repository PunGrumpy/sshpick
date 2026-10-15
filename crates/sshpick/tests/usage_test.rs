mod common;

use std::io;
use std::process::{Output, Stdio};

use assert_cmd::Command;
use common::{isolated, sshpick, sshpick_std, stderr};
use insta::assert_snapshot;

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
fn help_and_version_exit_zero_when_stdout_closes_early() {
    for args in [&["--help"][..], &["-V"][..], &["--list"][..]] {
        let out = with_stdout_closed("ui", args).unwrap();
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        assert_eq!(String::from_utf8_lossy(&out.stderr), "", "{args:?}");
    }
}

#[test]
fn help_goes_to_stdout_and_leads_with_the_version() {
    let out = sshpick("home").arg("--help").assert().code(0).stderr("");
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.starts_with("sshpick 0.1.0\n\n  ▌ sshpick [options] [command | query ...]\n"));
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
    assert!(list.starts_with("\n  ▌ sshpick list [query ...] [options]\n"));
    assert_eq!(list, page(&["ls", "-h"]));
    assert_eq!(list, page(&["help", "ls"]));
    assert_eq!(list, page(&["help", "list"]));
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
            .stdout("sshpick 0.1.0\n")
            .stderr("");
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
