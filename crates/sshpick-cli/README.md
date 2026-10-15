# sshpick-cli

## Purpose

The command line of sshpick: it parses argv, runs the command, prints what went wrong and picks
the exit code. The `sshpick` binary is one call into `sshpick_cli::main`. Rules live in
`sshpick-core`; everything the user sees is drawn by `sshpick-ui`.

## Architecture

```
sshpick-cli
    ├── cli/ - argv in, exit code out
    │   ├── spec.rs - the command table: commands, options, examples, the text of every help page
    │   ├── args.rs - Args, Command, UsageError, parse(): routing over the table
    │   ├── help.rs - render(): a help page as a pure function of the table, the width and colour
    │   ├── suggest.rs - closest(): a typo suggestion only for a unique single edit
    │   ├── error.rs - Error, the one exit_code() table, fix() and docs() for the Fix and Learn More lines, Exit
    │   └── test.rs - argv tables, exit codes, help snapshots
    └── commands/ - one file per command, each with its own Error; mod.rs holds CommandBase and
        the host view the JSON output shares
        ├── pick.rs - settle the query with Picker::resolve, or open the picker, then exec ssh
        ├── list.rs - a table on a terminal, tab-separated rows on a pipe, or --json
        └── inspect.rs - the one matching host, what ssh -G says, the command; text or --json
```

## Notes

Add a command or flag to `cli/spec.rs` first: parsing, `--help`, the root command list and
`sshpick help <command>` all read it. The first positional word selects a command when it names
one (`pick`, `list`, `ls`, `inspect`, `help`); anything else is `pick`'s query, and `--` ends options.

Cancel (130) and a quiet no-match (1) come back as `Ok(code)`, the way turbo returns
`Ok(exit_code)`; every failure is an `Error` and `Error::exit_code` is the only place a code is
chosen for one. Only the library prints, so the binary stays a few lines. Results go through
`Output::result()`, which treats a closed pipe as success, so `sshpick --help | head -1` exits 0.
