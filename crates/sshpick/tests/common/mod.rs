use std::path::{Path, PathBuf};
use std::process;

use assert_cmd::Command;
use assert_cmd::assert::Assert;

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// `sshpick` run against a fixture home, with the fake `ssh` first on `PATH` and none of the
/// caller's colour, width or sshpick settings, so a test sees only what it sets.
pub fn sshpick(home: &str) -> Command {
    Command::from_std(plain(home))
}

/// The same command as a `std` one, for a test that wires up stdout itself.
pub fn plain(home: &str) -> process::Command {
    let path = format!(
        "{}:{}",
        fixtures().join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut command = process::Command::new(env!("CARGO_BIN_EXE_sshpick"));
    command.env("HOME", fixtures().join(home)).env("PATH", path);
    for name in ["NO_COLOR", "CLICOLOR_FORCE", "COLORFGBG", "COLUMNS", "TERM"] {
        command.env_remove(name);
    }
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("SSHPICK_") {
            command.env_remove(name);
        }
    }
    command
}

/// What the command wrote to stderr, with the fixture directory replaced so a snapshot does not
/// depend on where the repository is checked out.
pub fn stderr(assert: &Assert) -> String {
    let text = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    text.replace(&fixtures().display().to_string(), "<fixtures>")
}
