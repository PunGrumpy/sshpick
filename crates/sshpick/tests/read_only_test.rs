mod common;

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use common::{fixtures, sshpick, stderr};

type Tree = BTreeMap<PathBuf, (u32, SystemTime, Vec<u8>)>;

fn ok<T, E: Debug>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("cannot {what}: {error:?}"),
    }
}

fn copy(from: &Path, to: &Path) {
    ok(fs::create_dir_all(to), "create a directory");
    for entry in ok(fs::read_dir(from), "read a fixture directory") {
        let entry = ok(entry, "read a fixture entry");
        let target = to.join(entry.file_name());
        if ok(entry.file_type(), "stat a fixture").is_dir() {
            copy(&entry.path(), &target);
        } else {
            ok(fs::copy(entry.path(), &target), "copy a fixture");
        }
    }
}

fn set_modes(path: &Path, dir: u32, file: u32) {
    if path.is_dir() {
        for entry in ok(fs::read_dir(path), "read a directory") {
            set_modes(&ok(entry, "read an entry").path(), dir, file);
        }
        ok(
            fs::set_permissions(path, Permissions::from_mode(dir)),
            "chmod",
        );
    } else {
        ok(
            fs::set_permissions(path, Permissions::from_mode(file)),
            "chmod",
        );
    }
}

fn tree(path: &Path, into: &mut Tree) {
    let meta = ok(fs::metadata(path), "stat");
    let contents = if meta.is_dir() {
        for entry in ok(fs::read_dir(path), "read a directory") {
            tree(&ok(entry, "read an entry").path(), into);
        }
        Vec::new()
    } else {
        ok(fs::read(path), "read a file")
    };
    let modified = ok(meta.modified(), "read the modification time");
    into.insert(
        path.to_owned(),
        (meta.permissions().mode(), modified, contents),
    );
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let root = std::env::temp_dir().join(format!("sshpick-read-only-{}", std::process::id()));
        copy(&fixtures().join("ui"), &root.join("home"));
        ok(
            fs::create_dir_all(root.join("cwd")),
            "create the working directory",
        );
        set_modes(&root, 0o555, 0o444);
        Scratch(root)
    }

    fn tree(&self) -> Tree {
        let mut into = Tree::new();
        tree(&self.0, &mut into);
        into
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        set_modes(&self.0, 0o755, 0o644);
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn read_only_ssh() -> PathBuf {
    fixtures().join("bin-read-only")
}

#[test]
fn the_fake_ssh_refuses_everything_but_g_and_v() {
    let ssh = read_only_ssh().join("ssh");
    for args in [&["-V"][..], &["-G", "--", "lab-a"][..]] {
        assert!(
            ok(Command::new(&ssh).args(args).output(), "run the fake")
                .status
                .success()
        );
    }
    let out = ok(
        Command::new(&ssh).args(["--", "lab-a"]).output(),
        "run the fake",
    );
    assert_eq!(out.status.code(), Some(99));
}

#[test]
fn ls_inspect_and_doctor_leave_a_read_only_home_and_directory_as_they_were() {
    let scratch = Scratch::new();
    let before = scratch.tree();
    let path = format!(
        "{}:{}",
        read_only_ssh().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    for args in [
        &["ls", "--json"][..],
        &["inspect", "lab-a", "--json"][..],
        &["doctor", "--json"][..],
    ] {
        let out = sshpick("ui")
            .args(args)
            .env("HOME", scratch.0.join("home"))
            .env("PATH", &path)
            .current_dir(scratch.0.join("cwd"))
            .assert()
            .code(0);
        assert_eq!(stderr(&out), "", "{args:?}");
        let stdout = &out.get_output().stdout;
        assert!(stdout.starts_with(b"{\n"), "{args:?} prints its JSON");
    }
    assert_eq!(
        scratch.tree(),
        before,
        "nothing was created, changed or removed"
    );
}
