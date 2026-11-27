# Picker

Press `?` on an empty filter, or `F1`, for the key list.

The picker opens on the alternate screen, as `less` and `vim` do, and gives the shell its screen
back, scrollback included, when it closes. It is a panel nearly the whole window, centred, with
one blank row above and below it. With an empty filter, in a terminal of 80 by 24:

```text
~/work $ keyway
╭──────────────────────────────────────────────────────────────────────────╮
│  Search host, ENV or what                                       5 hosts  │
│                                                                          │
│  lab-a           LAB — personal VMs                                 LAB  │
│  lab-b           build box                                          LAB  │
│  newstg-consul1  consul server 1 — mgmt                     NEW STAGING  │
│  newstg-consul2                                             NEW STAGING  │
│  newstg-bastion  bastion                                    NEW STAGING  │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│  ubuntu@10.11.212.191:22 via newstg-bastion         Keys ?    Connect ↵  │
╰──────────────────────────────────────────────────────────────────────────╯
```

## Keys

| Key                         | Effect                                        |
|-----------------------------|-----------------------------------------------|
| Enter                       | ssh to the selected host                      |
| Esc, Ctrl-C, Ctrl-G, Ctrl-Q | quit (exit 130); Esc closes the help first    |
| Up, Ctrl-P, Ctrl-K          | previous host; wraps to the last              |
| Down, Ctrl-N, Ctrl-J        | next host; wraps to the first                 |
| type                        | filter; 'exact ^prefix suffix$ !not           |
| Backspace, Ctrl-H           | delete the last character                     |
| Ctrl-U                      | clear the filter                              |
| Ctrl-W                      | delete the last word                          |
| Ctrl-D                      | quit when the filter is empty                 |
| ?, F1                       | toggle help; ? only on an empty filter        |

The help is a section of the panel, not a popup: opening it gives the keys the rows above the
rule, taken from the list, and closing it gives them back. The panel itself does not move. It
does not take over the keyboard: typing, moving and Enter still work while it is open, and Esc
closes it before a second Esc quits. The footer says `Close esc` meanwhile. If the keys do not
fit, the list keeps at least three lines and the last row of the help is
`+N more · keyway pick --help`. A key row is cut with `…` when the panel is too narrow to hold
it; `keyway pick --help` has the whole text. The arrow keys are in this list, not in the
footer.

The list is top-down with the best match first and the selection on it, like fzf's
`--layout=reverse`. The text cursor is always at the end of the query.

## Filter

Each word of the filter matches inside one column: `prod db` lists `prod-db-primary`, not
`prod-bastion` in the `PRODUCTION` file. `^` and `$` anchor the start and end of a column, so
`^prod-db` matches HOST and `^PROD` matches ENV. A word that no column of any host contains also
looks in the `HostName` and `User` written under each `Host` and in the target of a hopping
`RemoteCommand`, so `10.13.1.9`, `ubuntu` and `ubuntu@10.13.1.9` find the host. Those, like aliases, are never bold. [Search](commands.md#search)
has the full rules.

Config warnings are not drawn in the picker. `keyway` prints each one as a `! …` line on stderr
before the picker opens, once, on every path, so it is on the shell's screen when the picker
closes.

A picker that is killed (SIGINT, SIGTERM, SIGHUP or SIGQUIT) gives the shell its screen back and
puts the terminal back first, then ends by that signal, so the shell sees 130, 143, 129 or 131.

## Recent hosts

The picker remembers the hosts you connect to. With an empty filter they come first, the latest
first, and the rest follow in the order of the config. In a terminal of 80 by 24, after connecting
to `newstg-consul1` two hours ago, to `lab-b` three days ago and to `newstg-consul2` two weeks
ago:

```text
~/work $ kw
╭──────────────────────────────────────────────────────────────────────────╮
│  Search host, ENV or what                                       5 hosts  │
│                                                                          │
│  newstg-consul1  consul server 1 — mgmt                 2h  NEW STAGING  │
│  lab-b           build box                              3d          LAB  │
│  newstg-consul2                                         2w  NEW STAGING  │
│  lab-a           LAB — personal VMs                                 LAB  │
│  newstg-bastion  bastion                                    NEW STAGING  │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│  ubuntu@10.11.212.191:22 via newstg-bastion         Keys ?    Connect ↵  │
╰──────────────────────────────────────────────────────────────────────────╯
```

With a query the best match is still first. Last use only orders hosts that match equally well, so
`consul` across five ENV files opens on the consul host you used last. LAST says how long ago that
was: `now`, `5m`, `2h`, `3d`, `6w` or `1y`.

| You want                                     | Keys                                  |
|----------------------------------------------|---------------------------------------|
| the host you used last                       | Enter                                 |
| the second or third host you used            | Down, Enter. Down, Down, Enter        |
| the consul host you used last, among many    | `kw consul`, then Enter               |

History changes the order and nothing else. Which hosts match, how many, and whether a query
connects at once are the same with or without it.

A host is remembered when you connect at a terminal: with Enter, with an exact name, or with a
query that matches one host. `--non-interactive`, a pipe and `TERM=dumb` remember nothing, and
`inspect` and `doctor` never look. `ls` looks only with `--recent`, which lists hosts in this order.
[Configuration](configuration.md#recent-hosts) says where the history is kept and how to forget it.

## Appearance

The picker takes the alternate screen: the whole window, with the shell's screen and scrollback
kept aside until it closes. On it is a panel, drawn like a `rex.ui.pick` prompt in the Rex
terminal: a search field on top, the hosts below it, and the keys under a rule. The panel is a
rounded box, centred, with one blank row above and below it and at least two blank columns at
each side, no wider than 110 columns. Its size depends only on the terminal, never on what it
shows: typing, moving and opening the help change none of its frame. The rest of the screen is
blank.

A terminal under 60 columns or under 10 rows has no border: the panel is the whole terminal, no
margins and no 110-column cap — the cap is the bordered panel's.

Esc gives the shell its screen back as it was, with the cursor where the command left it. Enter
does the same, and `> ssh -- <host>` is printed there before ssh starts. Enter with nothing
matched exits 1 and says so. The picker never asks the terminal anything, such as where the
cursor is.

The panel reads top-down. The first row is the prompt, a strip a little lighter than the panel, with
the same margin at each side as the selection bar. On it, one column in, is the query, or the
placeholder `Search host, ENV or what` while the query is empty — the same text as
`integrations/rex/keyway.lua` — and at its right end the count: `5 hosts` on an empty filter, `2 /
5` of them matched on a query. The count is left out when the query — or, with an empty filter, the
placeholder — would not fit beside it. A panel with no room for the selected host shows its name on
the strip instead. A blank row follows, then the list, best match first. There is no header row.

Every host is one line: HOST at the left, WHAT after it, then LAST, then ENV right-aligned at the
far right. WHAT and LAST are muted, and a host with no WHAT leaves that span blank. When a line is
too narrow, WHAT is cut with `…` first, row by row. LAST goes, for every row at once, when the row
would leave WHAT fewer than eight columns — or fewer than the widest WHAT when that is shorter —
WHAT goes when fewer than that remain for it, and then HOST is cut with `…`. ENV is never left out.
The list scrolls by host and keeps the selected host in view.

The last row is the footer, under a rule. At its left is where the selected host goes, as `ssh -G`
resolves it: `ubuntu@192.0.2.11:22 via stg-bastion`, with `runs <command>` after it when the host
sets a RemoteCommand. Until ssh answers it reads `resolving…`; if ssh fails it reads `ssh -G
failed:` and the first line of what ssh said, and `keyway inspect <host>` has the rest. The route
runs from the row's indent to two columns before the keys and is cut with `…` when longer — the 45
and the 24 include that indent and that gap — and there is none when nothing matches. At the right
are the keys, each a label and then its key in a keycap, from left to right: `Keys ?`, `Quit esc`
and `Connect ↵`. Which keys are drawn is chosen from the row's width alone, so they never move with
the selection: the full set while the route's side keeps 45 columns, the labelled `Keys ?`/`Connect
↵` pair while it keeps 24, the bare keys (`? ↵`) otherwise. `?` is offered while the filter is empty
and `F1` once it is not. With the help open the label reads `Close esc`. The arrow keys are not in
the footer; the help lists them. With a query, in a terminal of 80 by 24:

```text
~/work $ keyway con
╭──────────────────────────────────────────────────────────────────────────╮
│  con                                                              2 / 5  │
│                                                                          │
│  newstg-consul1  consul server 1 — mgmt                     NEW STAGING  │
│  newstg-consul2                                             NEW STAGING  │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│  ubuntu@10.11.212.191:22 via newstg-bastion        Keys F1    Connect ↵  │
╰──────────────────────────────────────────────────────────────────────────╯
```

The text above cannot show the bar. The selected host is a filled bar, from the left margin to
the right one, across the whole line: the terminal's own background on blue (ANSI 4), so the
bar and its text contrast on every theme. Without colour (`NO_COLOR`, or a terminal that has
none) the bar is reverse video. The other colours come from the fixed xterm grey ramp
(232–255), so no look depends on what a theme calls black or white:

| Part                                                                                  | Colour                          | Without colour |
|---------------------------------------------------------------------------------------|---------------------------------|----------------|
| the HOST of unselected hosts                                                          | the default                     | the default    |
| the prompt strip                                                                      | dark grey (236) background      | the default    |
| the query, and the selected host's name on the strip                                  | light grey (255) on grey (236)  | the default    |
| the placeholder, and the count at the strip's right                                   | grey (245) on grey (236)        | dim            |
| WHAT, LAST and ENV of unselected hosts, the route, the footer's labels                | grey (245)                      | dim            |
| the border and the rule                                                               | grey (245), dimmer              | dim            |
| the selected host                                                                     | the background on blue (4)      | reverse        |
| a key in the footer                                                                   | light grey (250) on grey (236)  | bold           |
| `ssh -G failed: …`                                                                    | red (1)                         | the default    |

Matched characters are bold, in the colour of the text they sit in, whether that is HOST, WHAT or
ENV, on the bar or off it. A word is bold in every column it matches, so `con` is bold in the HOST
`newstg-consul1` and in its WHAT `consul server 1 — mgmt`. They are never dim. There is no other
bold except the keys.

When nothing matches, the first list row shows a muted `No host matches "zzzz"`. LAST is drawn
once some host has history.

A shorter terminal gets fewer rows of the same panel, never another layout. A terminal of ten
rows keeps the border: inside it are the prompt, three hosts and the footer. At nine rows the
border goes and every row is the picker's. Then the blank row under the prompt goes, then hosts,
and the footer outlasts every host but one. Past 110 columns a wider terminal only has wider
margins; a narrower one gives the panel
its width and cuts lines instead. A panel with no room for the selected host shows its name at
the right of the prompt row, so Enter is never blind. A resize clears the screen and draws it
again.

There is no animation or fade, by design. Each key press is one redraw.
