# sshpick

## Purpose

The `sshpick` binary. It is one call into `sshpick_cli::main` and holds the black-box tests that
run the built binary.

## Architecture

```
sshpick
    ├── src/main.rs - sshpick_cli::main() as an exit status
    └── tests/ - assert_cmd tests against the real binary
        ├── common/mod.rs - a fixture HOME, the fake ssh on PATH, a clean environment
        ├── pick_test.rs - one match, no terminal, no hosts, unreadable config
        ├── list_test.rs - the tab-separated rows of --list
        └── usage_test.rs - help, version, unknown options
```

## Notes

`main.rs` should not grow. New logic goes to `sshpick-cli`. The tests put `tests/fixtures/bin`
first on `PATH`, so nothing here runs the real `ssh`, and they read `tests/fixtures`, never
`~/.ssh`.
