# Agent guidelines

## Before pushing

- Run `make quality` (fmt, taplo, `cargo lint`, cargo-deny, tests). Do not use `--no-verify`.
- `unwrap()` and `expect()` are denied outside tests. Use `?`, `let ... else`, or a typed error.
- Tests read `crates/*/tests/fixtures`, never the real `~/.ssh` (it changes under you).

## Where code goes

- A rule (what a key does, how a line parses, how rows rank, when a preview is retried) goes in
  `crates/sshpick-core`, with a unit test. That crate must not depend on ratatui or crossterm.
- Anything that names a ratatui or crossterm type, and the mapping from terminal input to
  `sshpick_core::Key`, goes in `crates/sshpick-ui`. It draws `Picker::view` and holds no picker
  state. If you are adding an `if` about picker state there, it belongs in core.
- Argv, help, errors, exit codes and what each command does go in `crates/sshpick-cli`: `cli/` for
  parsing and errors, `commands/` for one file per command with its own `Error`. An exit code is
  chosen in `cli::Error::exit_code` and nowhere else.
- `crates/sshpick/src/main.rs` stays a few lines. New logic goes to `sshpick_cli::main`.
- Only `sshpick_core::ssh` runs `ssh`. Pass a `Host`, never a string from the user.
- A new crate needs its own dependency footprint or a second consumer, a `README.md` (Purpose,
  Architecture, Notes) and `[lints] workspace = true`.

## Tests

- Help text, usage errors and rendered frames are insta snapshots. Run
  `INSTA_UPDATE=always cargo test`, then read the `.snap` diffs before committing. CI never
  writes snapshots.
- Tables of inputs (argv, sizes, colour rules) use `test_case`.
- A snapshot holds text only. `REVERSED`, `BOLD` and `DIM` are asserted cell by cell.
- Tests that run the binary live in `crates/sshpick/tests` and use `assert_cmd` with the fake
  `ssh` from `tests/fixtures/bin` first on `PATH`.

## PR title format

PR titles must follow [Conventional Commits](https://www.conventionalcommits.org/). See
[`.github/workflows/lint-pr-title.yml`](./.github/workflows/lint-pr-title.yml) for the enforced
constraints.

Format: `<type>: <Description>`

- Description must start with an uppercase letter
- Scopes are not allowed

```
feat: Add Ctrl-W to delete the last word
fix: Keep the ENV label when line 1 has a BOM
```
