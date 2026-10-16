# sshpick

## Purpose

The `sshpick` binary and its short name `sp`. Each is one call into `sshpick_cli::main`. The crate
also holds the black-box tests that run the built binaries.

## Architecture

```
sshpick
    ├── src/main.rs - sshpick_cli::main() as an exit status
    ├── src/bin/sp.rs - the same call, installed as `sp`
    └── tests/ - assert_cmd tests against the real binary
        ├── common/mod.rs - a fixture HOME, the fake ssh on PATH, a clean environment
        ├── pick_test.rs - one match, no terminal, no hosts, unreadable config
        ├── list_test.rs - the tab-separated rows of ls and --list, and ls --json
        ├── inspect_test.rs - inspect as text and as JSON, and how it fails
        └── usage_test.rs - help, version, unknown options, and `sp`
```

## Notes

`main.rs` should not grow. New logic goes to `sshpick-cli`. The tests put `tests/fixtures/bin`
first on `PATH`, so nothing here runs the real `ssh`, and they read `tests/fixtures`, never
`~/.ssh`.
