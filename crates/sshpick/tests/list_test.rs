mod common;

use common::sshpick;

#[test]
fn list_prints_every_host_as_tsv_in_file_order() {
    sshpick("ui")
        .arg("--list")
        .assert()
        .code(0)
        .stdout(
            "LAB\tlab-a\tLAB — personal VMs\n\
             LAB\tlab-b\tbuild box\n\
             NEW STAGING\tnewstg-consul1\tconsul server 1 — mgmt\n\
             NEW STAGING\tnewstg-consul2\t\n\
             NEW STAGING\tnewstg-bastion\tbastion\n",
        )
        .stderr("");
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
