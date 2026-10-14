# Agent guidelines

## Before pushing

- Run `make quality` (fmt, taplo, `cargo lint`, cargo-deny, tests). Do not use `--no-verify`.
- `unwrap()` and `expect()` are denied outside tests. Use `?`, `let ... else`, or a typed error.
- Tests read `crates/*/tests/fixtures`, never the real `~/.ssh` (it changes under you).

## Where code goes

- A rule (what a key does, how a line parses, how rows rank, when a preview is retried) goes in
  `crates/sshpick-core`, with a unit test. That crate must not depend on ratatui or crossterm.
- `crates/sshpick` maps terminal input to `sshpick_core::Key`, draws `Picker::view`, and execs.
  If you are adding an `if` about picker state there, it belongs in core.
- Only `sshpick_core::ssh` runs `ssh`. Pass a `Host`, never a string from the user.

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
