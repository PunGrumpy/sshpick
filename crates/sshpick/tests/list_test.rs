mod common;

use common::{sshpick, stderr};

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
