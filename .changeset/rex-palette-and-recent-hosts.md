---
"keyway": minor
---

Pick hosts from Rex's palette, list recent hosts first, and draw the picker as a Rex-style panel.

### Added

- `keyway --print` prints the picked host instead of connecting, and `--print --json` prints its
  `{host, env, what, names}` object. The picker draws on `/dev/tty`, so `$(keyway --print)`
  captures only the host.
- `integrations/rex/keyway.lua` lists the hosts in Rex's command palette from a key and opens the
  picked host in a new tab labelled `host · ENV`, the hosts you connected to last first. A Rex app
  too old for prompts gets keyway's picker in a floating layer. It needs a keyway with
  `ls --recent`. See `docs/rex.md`.
- A host with no comment of its own takes the heading of its section as its WHAT: the first line
  of a comment block after a blank line, up to the next blank line or the next host with its own
  comment.
- The picker remembers the hosts you connect to at a terminal. It lists them first with an empty
  filter, breaks ties between equal matches by last use, and shows when in a LAST column.
  `XDG_STATE_HOME` moves the history from `~/.local/state/keyway`.
- `keyway ls --recent` lists hosts in the picker's order, the ones you connected to first, the
  latest first. It reads the history and never writes it, and changes the order only. Plain `ls`
  is unchanged.
- An address, part of one, a user or `user@address` from shell history finds a host, from the
  `HostName` and `User` written in its `Host` block, or the `user@target` of its `RemoteCommand`.

### Changed

- The picker is drawn like a `rex.ui.pick` prompt in the Rex terminal: a rounded panel nearly
  the whole window, centred, no wider than 110 columns, with the keys under a rule. Its size
  depends only on the terminal, so typing, moving and opening the key help move nothing but the
  selection bar and the text. A terminal narrower than 60 columns or shorter than 10 rows gets no
  border. The prompt is a strip a little lighter than the panel: the query or
  `Search host, ENV or what`, and the host count at its right. Every host is one line: HOST,
  WHAT, then LAST and ENV at the right. The selected host is a filled bar over the whole line
  (blue, or reverse video without colour), and matched characters are bold in the colour of their
  text, in WHAT as well as in HOST and ENV. There is no header row. The footer has the route at
  its left and the keys at its right, each a label and then a keycap: `Keys ?`, `Quit esc`,
  `Connect ↵`. The arrow keys are in the key help only. The colours are the fixed xterm grey
  ramp for the neutrals and ANSI 4 for the accent, so no look depends on what a theme calls
  black or white, and `NO_COLOR` turns them off.
- Each search word matches inside one column. `git`, `ru` or `db` no longer find hosts by letters
  spread across ENV, HOST and WHAT, so look-alike hosts in other environments drop out. A word no
  host contains still matches when each letter starts a word: `pdb` finds `prod-db-primary`.

### Fixed

- `^` and `$` anchor the start and end of each column, so `^prod-db` and `prod$` find hosts, and a
  capital `^PROD` or `API$` is no longer always empty.
- An ENV label line such as `# PROD: production, VPN only` shows `PROD`. Before, the whole line
  filled the ENV column and pushed WHAT off the screen.
