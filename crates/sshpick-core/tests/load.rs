use std::path::{Path, PathBuf};

use sshpick_core::{Warning, load};

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn rows_follow_ssh_read_order() {
    let loaded = load(&fixture_dir("home")).unwrap();
    let rows: Vec<(&str, &str, &str)> = loaded
        .entries
        .iter()
        .map(|e| (e.env.as_str(), e.host.as_str(), e.what.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("LAB", "lab-a", "LAB — personal VMs (direct, no bastion)"),
            ("LAB", "lab-b", "build box"),
            (
                "NEW STAGING",
                "new-bastion",
                "bastion: jump host for the stg network"
            ),
            (
                "NEW STAGING",
                "new-consul1",
                "indented comment wins (the last comment above Host)"
            ),
            ("NEW STAGING", "new-consul2", ""),
            ("NEW STAGING", "jumpbox", ""),
            ("OLD STAGING", "old-eq", "OLD STAGING — stg.example (kolla)"),
            ("OLD STAGING", "old-tab", ""),
            ("OLD STAGING", "old-indented", ""),
            ("OLD STAGING", "old-ok", ""),
            ("OLD STAGING", "old-trail", ""),
            ("OLD STAGING", "old-db", ""),
            ("nocomment", "nocomment-first", ""),
            ("CRLF", "solo-crlf", "solo"),
            ("LATE", "late", ""),
            ("", "root-host", "reachable from anywhere"),
        ]
    );
}

#[test]
fn a_self_including_glob_warns_once_and_does_not_fan_out() {
    let loaded = load(&fixture_dir("home")).unwrap();
    let [warning] = loaded.warnings.as_slice() else {
        panic!("one warning, not {:?}", loaded.warnings);
    };
    assert!(
        matches!(warning, Warning::Cycle { path } if path.ends_with("config.d/9-late.conf")),
        "the cycle names the file that includes itself: {warning:?}"
    );
    assert!(
        warning
            .to_string()
            .ends_with("config.d/9-late.conf: skipped, include cycle"),
        "the warning says what was skipped and why: {warning}"
    );
    let late = loaded.entries.iter().filter(|e| e.host.as_str() == "late");
    assert_eq!(late.count(), 1, "the cyclic file contributes its host once");
}

#[test]
fn include_deeper_than_sixteen_warns() {
    let loaded = load(&fixture_dir("deep")).unwrap();
    let hosts: Vec<_> = loaded.entries.iter().map(|e| e.host.to_string()).collect();
    let want: Vec<_> = (1..=16).map(|n| format!("h{n:02}")).collect();
    assert_eq!(hosts, want);
    assert!(matches!(
        loaded.warnings.as_slice(),
        [Warning::TooDeep { path }] if path.ends_with("chain/d17.conf")
    ));
}

#[test]
fn unreadable_root_is_the_only_error() {
    let error = load(Path::new("/nonexistent-home")).unwrap_err();
    assert!(error.to_string().contains("/nonexistent-home/.ssh/config"));
}

#[test]
fn an_unreadable_include_and_a_bad_glob_each_cost_one_include() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let home = std::env::temp_dir().join(format!("sshpick-load-{}", std::process::id()));
    let ssh = home.join(".ssh");
    fs::create_dir_all(&ssh).unwrap();
    fs::write(
        ssh.join("config"),
        "Include locked.conf\nInclude [\nHost kept\n",
    )
    .unwrap();
    let locked = ssh.join("locked.conf");
    fs::write(&locked, "Host locked-host\n").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let loaded = load(&home).unwrap();
    let root_reads_anyway = fs::read(&locked).is_ok();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_dir_all(&home).unwrap();
    if root_reads_anyway {
        return;
    }
    let hosts: Vec<_> = loaded.entries.iter().map(|e| e.host.as_str()).collect();
    assert_eq!(hosts, ["kept"], "each bad Include costs only itself");
    assert!(
        matches!(
            loaded.warnings.as_slice(),
            [Warning::Unreadable { path, .. }, Warning::BadInclude { arg, .. }]
                if path.ends_with("locked.conf") && arg == "["
        ),
        "{:?}",
        loaded.warnings
    );
}
