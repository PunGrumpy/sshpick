mod common;

use common::sshpick;

#[test]
fn one_match_execs_ssh_with_double_dash_and_no_tty() {
    sshpick("home")
        .assert()
        .success()
        .stdout("ssh -- only-host\n");
}

#[test]
fn the_picker_without_a_terminal_exits_two_and_leaves_stdout_alone() {
    sshpick("ui")
        .assert()
        .code(2)
        .stdout("")
        .stderr("sshpick: needs an interactive terminal; use --list to print the hosts\n");
}

#[test]
fn a_config_without_hosts_exits_two_for_the_picker_and_for_list() {
    for args in [&[][..], &["--list"][..]] {
        sshpick("empty")
            .args(args)
            .assert()
            .code(2)
            .stdout("")
            .stderr(
                "sshpick: no hosts found in ~/.ssh/config or the files it Includes\n\
                 Hosts come from 'Host' lines; see README for the ENV/WHAT comment conventions.\n",
            );
    }
}

#[test]
fn unreadable_config_exits_two() {
    sshpick("home")
        .env("HOME", "/nonexistent-home")
        .assert()
        .code(2);
}
