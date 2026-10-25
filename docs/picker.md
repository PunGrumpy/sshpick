# Picker

Press `?` on an empty filter, or `F1`, for the key list.

The picker after typing `consul`, at 80 columns. The prompt is the top row, with the count on
its right. The best match is the first row under the header, and the reversed `▌` row is the
selection. From 100 columns the preview sits in a pane beside the list instead of under it.

```text
> consul                                                                    4/12
  ENV     HOST          WHAT
▌ STAGING stg-consul1   consul server 1
  STAGING stg-consul2   consul server 2
  STAGING stg-consul3   consul server 3
  STAGING stg-consul-ui consul web UI tunnel: `ssh -f stg-consul-ui`, then open…


 stg-consul1 > ssh -G
  user          ubuntu
  hostname      192.0.2.11
  port          22
  proxyjump     stg-bastion

↑ ↓ - Select
F1 - More binds
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

The help is a popup over the list. It does not take over the keyboard: typing, moving and Enter
still work while it is open. The list is top-down with the best match first and the selection on
it, like fzf's `--layout=reverse`. The text cursor is always at the end of the query.

Config warnings are not drawn in the picker. `sshpick` prints each one as a `! …` line on stderr
before it takes the terminal, once, on every path, so it is still on the screen when the picker
closes.

A picker that is killed (SIGINT, SIGTERM, SIGHUP or SIGQUIT) puts the terminal back first, then
ends by that signal, so the shell sees 130, 143, 129 or 131.

## Appearance

The screen reads top-down. The first row is the prompt: `> `, the query, and on the right the
count of matches out of all hosts (`5/29`). A dim header follows, then the list, best match
first, and two dim footer rows. The first is `↑ ↓ - Select`, with `N more below` after it when
more rows match than fit. The second is `? - More binds`, or `F1 - More binds` once the query is
not empty.

The selected row is reversed video with a `▌` marker. Matched characters are bold. ENV, WHAT,
the header, the count, the footer, the titles and the preview keys are dim. Nothing has a
colour, so `NO_COLOR` has nothing to remove here.

At 100 columns or more the list has one border, on its right edge, and the preview is a
40-column pane beside it. The pane's dim title, `lab-a > ssh -G`, is on the prompt row. It ends
in `»` while ssh has not answered and in `failed` when ssh failed. The pane's dim footer, on the
last row, reads `Enter - ssh   Esc - Quit`, without Enter when nothing matches. Below 100
columns the preview is stacked under the list: a dim title row and five rows, and no pane
footer.

When nothing matches, the first list row shows a dim `no match`. An empty prompt shows a dim
`type to filter`. A WHAT wider than its column ends in `…`. HOST and ENV are never cut.

On a short terminal the picker gives the room to the list. Below 13 rows, at under 100
columns, the stacked preview is not drawn. Below 7 rows, at any width, the header and the
footer go too, which leaves the prompt and the list.

There is no animation or fade, by design. Each key press is one redraw. A preview that is
still loading shows its title and an empty body.
