mod common;

use common::sshpick;

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
