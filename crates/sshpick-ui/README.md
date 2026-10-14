# sshpick-ui

## Purpose

Everything sshpick shows: the colour decision for each output stream, console output, and the
picker. The picker owns raw mode and the alternate screen, draws with ratatui and maps crossterm
key events to `sshpick_core::Key`. This is the only crate that names a ratatui or crossterm type.
What a key does and which rows match live in `sshpick-core`; what to say lives in `sshpick-cli`.

## Architecture

```
sshpick-ui
    ├── color.rs - ColorConfig: colour and attributes for one stream, from NO_COLOR, CLICOLOR_FORCE, TERM
    ├── output.rs - Output: results on a stdout that forgives a closed pipe, errors and warnings on
    │   stderr; built once per run and passed down
    ├── text.rs - display-width helpers: columns, fit, truncate_to_width, plural
    └── tui/ - the picker
        ├── app.rs - the event loop: draw, poll, hand a key to the Picker
        ├── input.rs - crossterm key event to Key, one to one, no policy
        ├── screen.rs - lays the widgets out for one frame
        ├── size.rs - Tier: what the terminal's size leaves room for
        ├── table.rs - the prompt row, the header and the host rows
        ├── pane.rs - the ssh -G preview, beside the list or stacked under it
        ├── bar.rs - the status bar, the rules and the theme (the look the restyle replaces)
        ├── style.rs - DIM, MATCHED, and runs of matched text as styled spans
        ├── terminal.rs - Tty: enter, leave, and a Drop guard that restores the terminal
        └── testing.rs - test helpers: a TestBackend terminal, render(), frame()
```

## Notes

Layout is a pure function of the frame's area, so a resize needs no handling beyond the next
draw. Widths are display columns, never `str::len`. `tui/snapshots` holds the text of each
frame per tier and size; `REVERSED`, `BOLD` and `DIM` cannot appear in text, so the tests
check those cell by cell.

`tests/fixtures` holds copies of the `ui`, `flat` and `home` configs that the frame tests
read. Each crate reads its own fixtures, never the real `~/.ssh`.
