//! Changesets writes the release version to `package.json`, and `scripts/sync-version.ts`
//! copies it into `Cargo.toml`. A release whose binary reported another version would be
//! published under the wrong tag.

#[allow(dead_code)]
mod common;

use common::sshpick;

#[test]
fn version_is_the_one_changesets_wrote() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../package.json")).unwrap();
    let version = manifest["version"].as_str().unwrap();
    assert_eq!(
        version,
        env!("CARGO_PKG_VERSION"),
        "crates/sshpick/package.json and Cargo.toml disagree; run `bun scripts/sync-version.ts`"
    );
    sshpick("home")
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("sshpick {version}\n"));
}
