# sshpick-ui

## Purpose

The terminal side of sshpick: the colour decision for each output stream, console output, the `ls`
host table and the picker. The text of the help pages and of `inspect` is written in `sshpick-cli`,
with a `ColorConfig` from here. The picker owns raw mode and the alternate screen, draws with
ratatui and maps crossterm key events to `sshpick_core::Key`. This is the only crate that names a
ratatui or crossterm type. What a key does and which rows match live in `sshpick-core`; what to
say lives in `sshpick-cli`.

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
        ├── app.rs - the event loop: draw, poll, hand a key to the Picker
        ├── input.rs - crossterm key event to Key, one to one, no policy
        ├── screen.rs - lays the widgets out for one frame; the one border is the list's right edge
        ├── signals.rs - a thread that restores the terminal on SIGINT, SIGTERM, SIGHUP or SIGQUIT,
        │   then ends the process by that signal
        ├── size.rs - Tier: what the terminal's size leaves room for
        ├── table.rs - the prompt row, the header, the host rows and the hint row
        ├── pane.rs - the ssh -G preview: a borderless block with a dim title, beside the list or
        │   stacked under it
        ├── popup.rs - the key help over the middle of the frame: rows from BINDINGS
        ├── style.rs - DIM, BOLD, REVERSED, and runs of matched text as styled spans
        ├── terminal.rs - Tty: raw mode and the alternate screen, and the one idempotent restore
        │   that `leave`, `Drop`, the panic hook and the signal thread all end in
        └── testing.rs - test helpers: a TestBackend terminal, render(), frame()
```

## Notes

Layout is a pure function of the frame's area, so a resize needs no handling beyond the next
draw. Widths are display columns, never `str::len`. `tui/snapshots` holds the text of each
frame per tier and size, plus a text dump of where `REVERSED`, `BOLD` and `DIM` are for the two
reference screens; the other styles are asserted cell by cell. The picker uses no colour at
all, so `NO_COLOR` has nothing to turn off.

`tests/fixtures` holds copies of the `ui`, `flat` and `home` configs that the frame tests
read. Each crate reads its own fixtures, never the real `~/.ssh`.
