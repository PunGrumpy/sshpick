# sshpick

## Purpose

The `sshpick` binary and its short name `sp`. Each is one call into `sshpick_cli::main`. The crate
also holds the black-box tests that run the built binaries.

## Architecture

```
sshpick
    ├── src/main.rs - sshpick_cli::main() as an exit status
    ├── src/bin/sp.rs - the same call, installed as `sp`
    └── tests/ - tests against the real binary
        ├── common/mod.rs - a fixture HOME, the fake ssh on PATH, a clean environment
        ├── common/pty.rs - the binary on a pty, and a vt100 screen to read what it drew
        ├── pick_test.rs - one match, no terminal, no hosts, unreadable config
        ├── picker_test.rs - the picker on a pty: Esc, Enter, F1 and keys typed before the first frame
        ├── list_test.rs - the tab-separated rows of ls and --list, and ls --json
        ├── inspect_test.rs - inspect as text and as JSON, and how it fails
        └── usage_test.rs - help, version, unknown options, and `sp`
```

## Notes

`main.rs` should not grow. New logic goes to `sshpick-cli`. The tests put `tests/fixtures/bin`
first on `PATH`, so nothing here runs the real `ssh`, and they read `tests/fixtures`, never
`~/.ssh`. The pty tests wait for a condition on the emulator's screen, with a ten second ceiling,
and never sleep. The fake `ssh` says `COOKED` or `RAW` on stderr when its stdin is a terminal, so a
test can tell whether the picker restored the terminal before it ran.
