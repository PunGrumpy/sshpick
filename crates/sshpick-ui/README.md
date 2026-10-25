# sshpick-ui

## Purpose

The terminal side of sshpick: the colour decision for each output stream, console output, the `ls`
host table and the picker. The text of the help pages and of `inspect` is written in `sshpick-cli`,
with a `ColorConfig` from here. The picker owns raw mode and a block of rows under the command
line, draws with ratatui through its own backend and maps crossterm key events to
`sshpick_core::Key`. This is the only crate that names a ratatui or crossterm type. What a key does
and which rows match live in `sshpick-core`; what to say lives in `sshpick-cli`.

## Architecture

```
sshpick-ui
    ├── color.rs - ColorConfig: colour and attributes for one stream, from --no-color, NO_COLOR,
    │   CLICOLOR_FORCE and TERM, plus bold, dim, cyan, red, yellow and gray
    ├── output.rs - Output: results on a stdout that forgives a closed pipe; Error, Fix and Learn More
    │   lines and `!` warnings on stderr; built once per run and passed down
    ├── listing.rs - the host table `sshpick ls` prints on a terminal
    ├── text.rs - display-width helpers: columns, fit, plural
    └── tui/ - the picker
        ├── app.rs - the event loop: fit the block, draw, poll, hand a key to the Picker
        ├── inline.rs - Inline: the ratatui backend for the block, the only writer to the terminal
        │   while the picker is open: relative moves, rows erased one at a time, no query
        ├── plan.rs - plan(): how many rows the block takes and which, from the terminal's size,
        │   the number of hosts and the open help; one ladder of what goes first
        ├── input.rs - crossterm key event to Key, one to one, no policy
        ├── screen.rs - lays out one block: prompt, header, list, route row, help, hint row
        ├── signals.rs - a thread that restores the terminal on SIGINT, SIGTERM, SIGHUP or SIGQUIT,
        │   then ends the process by that signal
        ├── table.rs - the prompt row, the header, the host rows and the hint row
        ├── route.rs - the route row: where the selected host goes, `resolving…` or why ssh -G failed
        ├── help.rs - the key help as rows between the route row and the hint row, which make the
        │   block taller: rows from BINDINGS, cut with a `+N more · sshpick pick --help` pointer
        ├── style.rs - DIM, BOLD, REVERSED, and runs of matched text as styled spans
        ├── terminal.rs - Tty: raw mode and the block, and the one idempotent restore that
        │   `leave`, `Drop`, the panic hook and the signal thread all end in
        └── testing.rs - test helpers: a TestBackend block for a terminal size, render(), frame()
```

## Notes

The block's rows come from `plan.rs` and its layout is a pure function of that plan. A resize, or
the help opening or closing, erases the block and draws it again whole from its top row, where the
cursor waits between frames. Widths are display columns, never `str::len`. `tui/snapshots` holds
the text of each frame per size, plus a text dump of where `REVERSED`, `BOLD` and `DIM` are for the
two reference screens; the other styles are asserted cell by cell. The picker uses no colour at
all, so `NO_COLOR` has nothing to turn off.

`tests/fixtures` holds copies of the `ui`, `flat` and `home` configs that the frame tests
read. Each crate reads its own fixtures, never the real `~/.ssh`.
