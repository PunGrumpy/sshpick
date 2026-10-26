# Picker

Press `?` on an empty filter, or `F1`, for the key list.

The picker opens on the whole screen, as `less` and `vim` do, and gives the shell its screen back,
scrollback included, when it closes. With an empty filter, in a terminal of 80 by 12:

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

The help is a section of the screen, not a popup: opening it takes the blank rows under the list
first, with the keys just above the hint row, and closing it gives them back. It does not take
over the keyboard: typing, moving and Enter still work while it is open, and Esc closes it before
a second Esc quits. The hint row says `esc close` meanwhile. If the keys do not fit, the list
keeps at least three rows and the last row of the help is `+N more · sshpick pick --help`.

The list is top-down with the best match first and the selection on it, like fzf's
`--layout=reverse`. The text cursor is always at the end of the query.

Config warnings are not drawn in the picker. `sshpick` prints each one as a `! …` line on stderr
before the picker opens, once, on every path, so it is on the shell's screen when the picker
closes.

A picker that is killed (SIGINT, SIGTERM, SIGHUP or SIGQUIT) gives the shell its screen back and
puts the terminal back first, then ends by that signal, so the shell sees 130, 143, 129 or 131.

## Appearance

The picker takes the alternate screen: the whole window, with the shell's screen and scrollback
kept aside until it closes. The prompt is on the top row, the route and the hint row are on the
bottom two, and the list fills the rows between. Rows the hosts do not need stay blank, so a short
config, or a query that matches fewer hosts, leaves blank rows above the route.

Esc gives the shell its screen back as it was, with the cursor where the command left it. Enter
does the same, and `> ssh -- <host>` is printed there before ssh starts. Enter with nothing
matched exits 1 and says so. The picker never asks the terminal anything, such as where the
cursor is.

The screen reads top-down. The first row is the prompt: `> `, the query, and on the right the count
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

From 100 columns the route takes no row. It moves into a panel beside the header and the list,
two fifths of the width wide, behind a dim rule: the selected host, then one row per key, with the
names and in the order `sshpick inspect` prints under Resolved. `resolving…` reads the same as on
the row, and a failure wraps over the panel's rows, ending in `…` if it does not fit. The panel
needs six rows beside it, so a terminal shorter than 8 rows keeps the route row. With a query, in
a terminal of 100 by 12:

```text
~/work $ sshpick consul
> consul                                                                                         2/5
  ENV         HOST           WHAT                           │ newstg-consul1
▌ NEW STAGING newstg-consul1 consul server 1 — mgmt         │ user       ubuntu
  NEW STAGING newstg-consul2                                │ hostname   10.11.212.191
                                                            │ port       22
                                                            │ proxyjump  newstg-bastion
                                                            │
                                                            │
                                                            │
                                                            │
                                                            │
  ↑↓ move   enter ssh   esc quit   F1 keys
```

The selected row is reversed video with a `▌` marker. Matched characters are bold. ENV, WHAT,
the header, the count, the arrow and the words `via` and `runs`, the hint row, and the panel's
rule and keys are dim. Nothing has a colour, so `NO_COLOR` has nothing to remove here.

When nothing matches, the first list row shows a dim `no matching hosts`. An empty prompt shows
a dim `type to filter`. A WHAT wider than its column ends in `…`, and one too narrow for more
than that is left out. HOST and ENV are never cut.

A shorter terminal gets fewer rows of the same screen, never another layout: the header goes
first, then the route row, then hosts, and the hint row with `enter` and `esc` outlasts every host
but one. Width changes one thing: under 100 columns the panel is the route row again. A screen of
one row shows the selected host where the count was, so Enter is never blind. A resize clears the
screen and draws it again.

There is no animation or fade, by design. Each key press is one redraw.
