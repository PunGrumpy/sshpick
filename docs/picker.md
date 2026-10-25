# Picker

Press `?` on an empty filter, or `F1`, for the key list.

The picker opens under the command line, in a block of rows, and the rest of the screen stays as
it was. With an empty filter, at 80 columns:

```text
~/work $ sshpick
> type to filter                                                             5/5
  ENV         HOST           WHAT
▌ LAB         lab-a          LAB — personal VMs
  LAB         lab-b          build box
  NEW STAGING newstg-consul1 consul server 1 — mgmt
  NEW STAGING newstg-consul2
  NEW STAGING newstg-bastion bastion
  → ubuntu@10.11.212.191:22 via newstg-bastion
  ↑↓ move   enter ssh   esc quit   ? keys
```

## Keys

| Key                         | Effect                                       |
|-----------------------------|----------------------------------------------|
| Enter                       | ssh to the selected host                     |
| Esc, Ctrl-C, Ctrl-G, Ctrl-Q | quit (exit 130); Esc closes this help first  |
| Up, Ctrl-P, Ctrl-K          | select the previous host; wraps to the last  |
| Down, Ctrl-N, Ctrl-J        | select the next host; wraps to the first     |
| type                        | filter; 'exact ^prefix suffix$ !not          |
| Backspace, Ctrl-H           | delete the last character                    |
| Ctrl-U                      | clear the filter                             |
| Ctrl-W                      | delete the last word                         |
| Ctrl-D                      | quit when the filter is empty                |
| ?, F1                       | toggle this help (? only on an empty filter) |

The help is a section of the block, not a popup: opening it makes the block taller, with the keys
below the route row and above the hint row, and closing it gives the rows back. It does not take
over the keyboard: typing, moving and Enter still work while it is open, and Esc closes it before
a second Esc quits. The hint row says `esc close` meanwhile. If the keys do not fit, the list
keeps at least three rows and the last row of the help is `+N more · sshpick pick --help`.

The list is top-down with the best match first and the selection on it, like fzf's
`--layout=reverse`. The text cursor is always at the end of the query.

Config warnings are not drawn in the picker. `sshpick` prints each one as a `! …` line on stderr
before the block opens, once, on every path, so it sits above the block and is still there when
the picker closes.

A picker that is killed (SIGINT, SIGTERM, SIGHUP or SIGQUIT) erases the block and puts the
terminal back first, then ends by that signal, so the shell sees 130, 143, 129 or 131.

## Appearance

The picker is a block of rows on the line under the command, where the cursor was. It takes a row
each for the prompt, the header, the route and the hint, and one per host, but at most half the
terminal (at least 8 rows when the terminal has them) and never all of it, so the command that
started it stays in sight. It is sized from every host, not from the matches, so it keeps its
height while you type, and fewer matches leave blank rows above the route. Started at the bottom
of the screen, it scrolls the lines above it up to make room; they stay in the scrollback.

Esc erases the block and leaves the cursor where the block began, so the shell's next prompt takes
that row. Enter erases it too, and `> ssh -- <host>` takes that row before ssh starts. Enter with
nothing matched exits 1 and says so. The picker never asks the terminal where the cursor is: it
assumes the command line ended with a new line, as a shell's does.

The block reads top-down. The first row is the prompt: `> `, the query, and on the right the count
of matches out of all hosts (`5/29`). A dim header follows, then the list, best match first. The
last row is one dim hint row: `↑↓ move   enter ssh   esc quit   ? keys`. The row shows the widest
form that fits and always keeps `esc`, and `enter` whenever something matches. A narrower
terminal loses `? keys` first, then `↑↓ move`, then the words (`enter esc`). `?` is offered while
the filter is empty and `F1` once it is not. With the help open the row says `esc close`. When
more rows match than fit, the range on screen (`1–8`) sits at the right edge of the row, and goes
before any key does.

The row above it says where the selected host goes, as `ssh -G` resolves it:
`→ ubuntu@192.0.2.11:22 via stg-bastion`, with `runs <command>` after it when the host sets a
RemoteCommand. Until ssh answers it reads `→ resolving…`. If ssh fails it reads
`→ ssh -G failed:` and the first line of what ssh said; `sshpick inspect <host>` has the rest. The
row is cut with `…` at the edge of the terminal and never wraps, and it is blank when nothing
matches.

The selected row is reversed video with a `▌` marker. Matched characters are bold. ENV, WHAT,
the header, the count, the arrow and the words `via` and `runs`, and the hint row are dim. Nothing
has a colour, so `NO_COLOR` has nothing to remove here.

When nothing matches, the first list row shows a dim `no matching hosts`. An empty prompt shows
a dim `type to filter`. A WHAT wider than its column ends in `…`, and one too narrow for more
than that is left out. HOST and ENV are never cut.

A shorter terminal gets fewer rows of the same block, never another layout: the header goes
first, then the route row, then hosts, and the hint row with `enter` and `esc` outlasts every host
but one. A block of one row shows the selected host where the count was, so Enter is never blind. A resize, or the help opening or closing, erases the block and draws it again from
the same row.

There is no animation or fade, by design. Each key press is one redraw.
