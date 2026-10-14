use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn sshpick_in(home: &str, args: &[&str]) -> io::Result<Output> {
    let path = format!(
        "{}:{}",
        fixtures().join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new(env!("CARGO_BIN_EXE_sshpick"))
        .args(args)
        .env("HOME", fixtures().join(home))
        .env("PATH", path)
        .stdin(Stdio::null())
        .output()
}

fn sshpick(args: &[&str]) -> io::Result<Output> {
    sshpick_in("home", args)
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn version_and_help_exit_zero() {
    assert!(sshpick(&["--version"]).unwrap().status.success());
    assert!(sshpick(&["-h"]).unwrap().status.success());
}

#[test]
fn one_match_execs_ssh_with_double_dash_and_no_tty() {
    let out = sshpick(&[]).unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "ssh -- only-host\n");
}

#[test]
fn an_unknown_option_exits_two_with_a_hint() {
    let out = sshpick(&["--bogus"]).unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "");
    assert_eq!(
        stderr(&out),
        "sshpick: unknown option '--bogus'\nRun 'sshpick --help' for usage.\n"
    );
}

#[test]
fn list_prints_every_host_as_tsv_in_file_order() {
    let out = sshpick_in("ui", &["--list"]).unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stdout(&out),
        "LAB\tlab-a\tLAB — personal VMs\n\
         LAB\tlab-b\tbuild box\n\
         NEW STAGING\tnewstg-consul1\tconsul server 1 — mgmt\n\
         NEW STAGING\tnewstg-consul2\t\n\
         NEW STAGING\tnewstg-bastion\tbastion\n"
    );
    assert_eq!(stderr(&out), "");
}

#[test]
fn list_filters_with_all_words_and_ranks_best_first() {
    let out = sshpick_in("ui", &["-l", "new", "consul"]).unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stdout(&out),
        "NEW STAGING\tnewstg-consul1\tconsul server 1 — mgmt\n\
         NEW STAGING\tnewstg-consul2\t\n"
    );
}

#[test]
fn list_keeps_three_columns_when_env_is_empty() {
    let out = sshpick_in("flat", &["--list"]).unwrap();
    assert_eq!(
        stdout(&out),
        "\troot-a\treachable from anywhere\n\troot-b\t\n"
    );
    let out = sshpick_in("tabs", &["--list"]).unwrap();
    assert_eq!(
        stdout(&out),
        "\ttabbed\tone two\n",
        "a tab in WHAT is a space"
    );
}

#[test]
fn list_with_no_match_prints_nothing_and_exits_one() {
    let out = sshpick_in("ui", &["--list", "zzz"]).unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stdout(&out), "");
    assert_eq!(stderr(&out), "");
}

#[test]
fn the_picker_without_a_terminal_exits_two_and_leaves_stdout_alone() {
    let out = sshpick_in("ui", &[]).unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stdout(&out), "", "no alt-screen escape reaches a pipe");
    assert_eq!(
        stderr(&out),
        "sshpick: needs an interactive terminal; use --list to print the hosts\n"
    );
}

#[test]
fn a_config_without_hosts_exits_two_for_the_picker_and_for_list() {
    for args in [&[][..], &["--list"][..]] {
        let out = sshpick_in("empty", args).unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(stdout(&out), "", "{args:?}");
        assert_eq!(
            stderr(&out),
            "sshpick: no hosts found in ~/.ssh/config or the files it Includes\n\
             Hosts come from 'Host' lines; see README for the ENV/WHAT comment conventions.\n",
            "{args:?}"
        );
    }
}

#[test]
fn unreadable_config_exits_two() {
    let out = Command::new(env!("CARGO_BIN_EXE_sshpick"))
        .env("HOME", "/nonexistent-home")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}
