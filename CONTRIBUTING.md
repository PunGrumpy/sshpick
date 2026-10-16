Thank you for your interest in contributing to sshpick!

- [General dependencies](#general-dependencies)
- [Structure of the repository](#structure-of-the-repository)
- [Building sshpick](#building-sshpick)
- [Running tests](#running-tests)
- [Opening a pull request](#opening-a-pull-request)

## General dependencies

You will need these installed to work on this repository:

- [Rust](https://www.rust-lang.org/tools/install) 1.99. rustup picks it up from
  [`rust-toolchain.toml`](rust-toolchain.toml).
- [taplo](https://taplo.tamasfe.dev) and [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
  for `make quality`. On macOS: `brew install taplo cargo-deny`.

## Structure of the repository

- `crates/sshpick-core` holds every rule: the config reader, ranking, the picker state and
  `ssh -G`. It cannot depend on a terminal crate, and `cargo deny` enforces that.
- `crates/sshpick-ui` is the terminal and everything sshpick prints.
- `crates/sshpick-cli` is argv, help, errors, exit codes and one file per command.
- `crates/sshpick` is the `sshpick` and `sp` binaries, a few lines each, and the tests that run
  them.
- `docs` is the user documentation.

Each crate has a `README.md` with its own layout. [AGENTS.md](AGENTS.md) has the rules for where
new code goes.

## Building sshpick

```bash
make setup     # once per clone: enables the pre-push hook
make install   # builds and installs sshpick and sp to ~/.local/bin
make fix       # cargo fmt and taplo format
```

## Running tests

```bash
make quality   # fmt, taplo, clippy -D warnings, cargo-deny and every test; CI runs the same
```

Tests read `crates/*/tests/fixtures`, never your `~/.ssh`, and put a fake `ssh` first on `PATH`.
Help text, usage errors and rendered frames are [insta](https://insta.rs) snapshots. After a
change, run `INSTA_UPDATE=always cargo test` and read the `.snap` diffs before you commit.

## Opening a pull request

Fill in the pull request template. Titles follow
[Conventional Commits](https://www.conventionalcommits.org/) without a scope, and the subject
starts with a capital letter:

```
feat: Add Ctrl-W to delete the last word
fix: Keep the ENV label when line 1 has a BOM
```

A change to a command name, flag, exit code, environment variable or TSV column needs a test and
a line in `docs`.
