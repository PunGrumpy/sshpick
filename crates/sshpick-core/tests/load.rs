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
fn self_include_warns_and_keeps_rows() {
    let loaded = load(&fixture_dir("home")).unwrap();
    assert!(matches!(
        loaded.warnings.as_slice(),
        [Warning::Cycle { path }] if path.ends_with("config.d/9-late.conf")
    ));
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
