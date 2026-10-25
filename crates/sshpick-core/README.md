# sshpick-core

## Purpose

sshpick's domain, with no terminal in it: which hosts `~/.ssh/config` defines, how a query ranks
them, what each key does, and what `ssh -G` says about a host. This crate cannot name a terminal
type because it does not depend on one, and `cargo deny` fails if `ratatui` or `crossterm` is ever
added to it. `sshpick-ui` owns the terminal (raw mode, drawing, reading keys); `sshpick-cli`
decides what to say.

## Architecture

```
sshpick-core
    ├── config.rs - load(): ~/.ssh/config and every Include in ssh's read order, as Entry rows plus
    │   Warnings; Host, a literal alias, is the only thing that ever reaches ssh
    ├── label.rs - env_label(): the ENV column, from line 1 of an included file or its file name
    ├── rank.rs - Ranker: nucleo's fuzzy match over ENV, HOST, WHAT and aliases, with the matched
    │   ranges per column
    ├── picker.rs - Picker: query, matches, cursor and scroll; resolve() settles a query before any
    │   terminal, handle(Key) returns an Outcome, follow(height) scrolls to the cursor and view()
    │   is the rows of one frame, with the scroll offset and whether the help is open
    ├── hints.rs - hint(): the one hint row, the widest rung of keys that fits, accept and cancel
    │   always kept, and the window range when it fits beside them
    ├── keymap.rs - BINDINGS: every key Picker::handle acts on, which the help, `sshpick pick
    │   --help` and the docs/picker.md table read; tests tie it to the picker
    ├── preview.rs - Previews: one worker thread that runs ssh -G, and a Slot per host
    └── ssh.rs - resolve(): ssh -G with a timeout, as a Preview or a ResolveError; Preview::route()
        and ResolveError::headline() are the one row the picker shows; command() and exec():
        ssh -- <host>
```

## Notes

Only `ssh.rs` runs `ssh`, and only with a `Host`, never a string from the user. `Picker` owns the
query, the cursor and the scroll; the UI keeps no picker state and builds each frame from
`Picker::view`. A rule goes here with a unit test; anything that names a terminal type goes in
`sshpick-ui`.

`tests/fixtures` holds synthetic configs (`home`, `deep`) and fake `ssh` programs (`bin`). Each
crate reads its own fixtures, never the real `~/.ssh`.
