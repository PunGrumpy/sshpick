# sshpick-ui

## Purpose

The terminal for sshpick: raw mode and the alternate screen, drawing the picker with ratatui,
and mapping crossterm key events to `sshpick_core::Key`. This is the only crate that names a
ratatui or crossterm type. What a key does and which rows match live in `sshpick-core`.

## Architecture

```
sshpick-ui
    └── tui/ - the picker
        ├── app.rs - the event loop: draw, poll, hand a key to the Picker
        ├── input.rs - crossterm key event to Key, one to one, no policy
        ├── screen.rs - layout and drawing
        └── terminal.rs - Tty: enter, leave, and a Drop guard that restores the terminal
```

## Notes

`tests/fixtures` holds copies of the `ui`, `flat` and `home` configs that the frame tests
read. Each crate reads its own fixtures, never the real `~/.ssh`.
