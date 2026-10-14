mod common;

use std::io;
use std::process::{Output, Stdio};

use common::{plain, sshpick};

fn with_stdout_closed(home: &str, args: &[&str]) -> io::Result<Output> {
    let (reader, writer) = io::pipe()?;
    drop(reader);
    plain(home)
        .args(args)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
}

#[test]
fn version_and_help_exit_zero() {
    sshpick("home").arg("--version").assert().success();
    sshpick("home").arg("-h").assert().success();
}

#[test]
fn an_unknown_option_exits_two_with_a_hint() {
    sshpick("home")
        .arg("--bogus")
        .assert()
        .code(2)
        .stdout("")
        .stderr("sshpick: unknown option '--bogus'\nRun 'sshpick --help' for usage.\n");
}

#[test]
fn help_and_version_exit_zero_when_stdout_closes_early() {
    for args in [&["--help"][..], &["-V"][..], &["--list"][..]] {
        let out = with_stdout_closed("ui", args).unwrap();
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        assert_eq!(String::from_utf8_lossy(&out.stderr), "", "{args:?}");
    }
}
