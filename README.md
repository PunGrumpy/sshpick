# sshpick

Pick a host from `~/.ssh/config` and ssh to it: a full-screen picker with fzf's keys and
ranking, and a live `ssh -G` preview of where the host really resolves.

    sshpick                 # every host
    sshpick consul          # pre-filtered; exactly one match connects without opening the picker
    sshpick new consul      # words are AND terms; fzf syntax: 'exact ^prefix suffix$ !not
    sshpick ls consul       # the matching hosts: a table on a terminal, tab-separated in a pipe
    sshpick help ls         # help for one command

Press `?` on an empty filter, or `F1`, for the key list.

The picker after typing `consul`, at 80 columns. The prompt is the top row, with the count on
its right. The best match is the first row under the header, and the `▌` row is the selection.
From 100 columns the preview sits in a pane beside the list instead of under it.

```text
> consul                                                                    4/12
────────────────────────────────────────────────────────────────────────────────
  ENV     HOST          WHAT
▌ STAGING stg-consul1   consul server 1
  STAGING stg-consul2   consul server 2
  STAGING stg-consul3   consul server 3
  STAGING stg-consul-ui consul web UI tunnel: `ssh -f stg-consul-ui`, then open…
─ ssh -G stg-consul1 ───────────────────────────────────────────────────────────
  user          ubuntu
  hostname      192.0.2.11
  port          22
  proxyjump     stg-bastion

 Enter ssh  Esc quit  Ctrl-U clear  ↑↓ move                            12 hosts
```

## What it shows

| Column | Where it comes from |
|--------|---------------------|
| ENV    | line 1 of each included file: `# OLD STAGING — anything` shows `OLD STAGING`; no label: the file name (`30-prod.conf` shows `prod`); hosts in `~/.ssh/config` itself: blank (the column hides when all are) |
| HOST   | the first name on the `Host` line, the one you would type; `*`/`?` patterns and `!negations` are skipped; the other names are searchable but not shown |
| WHAT   | the `#` comment on the line directly above `Host` |

The preview runs `ssh -G -- <host>` and shows `user`, `hostname`, `port`, `remotecommand`,
`proxyjump`, so `Match`, `ProxyJump` and `%` tokens show what ssh will actually do. A broken
config shows ssh's own error there. Each host is resolved once per session.

### Appearance

The screen reads top-down. The first row is the prompt: `> `, the query, and on the right the
count of matches out of all hosts (`5/29`). A dim rule, the header, then the list, best match
first. A second dim rule closes the list, and the last row is the status bar.

The selected row is an inverted bar with a `▌` marker. Matched characters are bold. ENV, WHAT,
the header, the count, the rules and the preview keys are dim. Nothing else has a colour.

At 100 columns or more the preview is a 40-column pane to the right of the list, behind a dim
`│`. Its title, `ssh -G host`, sits on the header row, and the values start on the next row.
Below 100 columns the preview is stacked under the list: the closing rule carries the title
(`─ ssh -G host ─`) and five rows follow.

When more rows match than fit, the closing rule says so on its right:
`──── 17 more below ─`. When nothing matches, the first list row shows a dim `no match`.

The last row is a status bar. It lists the main keys on the left. On the right it shows the
host count. If the config has warnings, the count is followed by the number of warnings and
the first one, cut to fit: `29 hosts · 2 warnings · <first warning>`. On a narrow terminal the
hints drop from the right.

An empty prompt shows a dim `type to filter`. A WHAT wider than its column ends in `…`. HOST
and ENV are never cut.

On a short terminal the picker gives the room to the list. Below 13 rows, at under 100
columns, the stacked preview is not drawn. Below 7 rows, at any width, the header, both rules
and the side pane go too, which leaves the prompt, the list and the status bar.

In the picker the bar's colours follow the terminal theme. `SSHPICK_THEME=dark` or `light` overrides the
detection, which otherwise reads `COLORFGBG` and falls back to dark. With `NO_COLOR` set, the
picker draws with reverse video, bold and dim only. `CLICOLOR_FORCE` turns colours back on.

There is no animation, spinner or fade, by design. Each key press is one redraw. A preview
that is still loading shows its title and an empty body.

## Config conventions

sshpick reads `~/.ssh/config` and follows its `Include` lines in the order ssh does. Two
comment conventions fill the columns. Nothing else in the config needs to change.

```text
# STAGING — shared test environment (rotates quarterly)
# jump host for the staging network
Host stg-bastion
  HostName 192.0.2.10

# consul server 1
Host stg-consul1 consul1.stg
  HostName 192.0.2.11
  ProxyJump stg-bastion
```

- Line 1 of an included file is the ENV label. Text from the first ` — `, ` - ` or ` (` on is
  dropped, so this file's label is `STAGING`.
- A comment directly above a `Host` line is its WHAT. A blank line or any other line in
  between clears it. Here `stg-bastion` gets `jump host for the staging network`. A `Host`
  on line 2 would get the label line as its WHAT too.
- `stg-consul1` is the row. `consul1.stg` is searchable, so typing it finds the row.
- `Host` follows ssh syntax: any case, tabs, `Host=x`, indentation. A name listed twice
  makes one row.

## Search

Words are AND terms and use fzf's extended syntax with smart case (a capital makes that word
case-sensitive).

| Term     | Matches                  |
|----------|--------------------------|
| `consul` | fuzzy                    |
| `'consul`| exact substring          |
| `^stg`   | prefix                   |
| `1$`     | suffix                   |
| `!ui`    | rows without `ui`        |

There is no `a | b` OR. The query is matched against ENV, HOST, WHAT and the other names on
the `Host` line.

An argument that starts with `-` is an option. To search for text that starts with `-`, put
`--` before it: `sshpick -- -bastion`. Everything after `--` is query text.

## Commands

| Command                  | What it does                                                      |
|--------------------------|-------------------------------------------------------------------|
| `pick [query ...]`       | The default. Connect when one host matches, else open the picker  |
| `list`, `ls [query ...]` | Print the hosts that match, best first                            |
| `inspect query ...`      | Show where the one matching host resolves, as `ssh -G` sees it    |
| `help [command]`         | Show help for a command; `sshpick help ls` is `sshpick ls --help` |

`pick`, `list`, `ls`, `inspect` and `help` are command words when they come first. Any other first word is
query text, so `sshpick consul` still searches. A host named exactly like a command word is
reached with `sshpick pick list` or `sshpick -- list`; when `list` or `inspect` is also a host or
an alias, a terminal gets a note saying so. `-h` and `--help` work anywhere and win
over every other error. `--version` and `-V` print the version; `-v` is the same flag.

Options go anywhere before a bare `--`:

| Option              | Effect                                                                 |
|---------------------|------------------------------------------------------------------------|
| `-h`, `--help`      | Show help; wins over every other error                                 |
| `-V`, `--version`   | Print the version (when no command is named)                           |
| `--no-color`        | Same as `NO_COLOR=1`                                                   |
| `--non-interactive` | Never open the picker: one match connects, none exits 1, several exit 2 |

## Keys

| Key                         | Effect                                       |
|-----------------------------|----------------------------------------------|
| Enter                       | ssh to the selected host                     |
| Esc, Ctrl-C, Ctrl-G, Ctrl-Q | quit (exit 130); Esc closes this help first  |
| Up, Ctrl-P, Ctrl-K          | select the previous host (up the list)       |
| Down, Ctrl-N, Ctrl-J        | select the next host (down the list)         |
| type                        | filter; 'exact ^prefix suffix$ !not          |
| Backspace, Ctrl-H           | delete the last character                    |
| Ctrl-U                      | clear the filter                             |
| Ctrl-W                      | delete the last word                         |
| Ctrl-D                      | quit when the filter is empty                |
| ?, F1                       | toggle this help (? only on an empty filter) |

The help is a popup over the list. It does not take over the keyboard: typing, moving and Enter
still work while it is open. The list is top-down with the best match first and the selection on
it, like fzf's `--layout=reverse`. The text cursor is always at the end of the query.

## Inspect

`sshpick inspect` is a dry run of `pick`: it takes the same query, needs exactly one match, and
shows what `ssh -G` says about the host and the command that would connect. Nothing is
connected.

    $ sshpick inspect stg-consul1
      General

        host   stg-consul1
        env    STAGING
        what   consul server 1
        names  stg-consul1, consul1.stg

      Resolved

        user       ubuntu
        hostname   192.0.2.11
        port       22
        proxyjump  stg-bastion

      Command

        ssh -- stg-consul1

`names` is the first name on the `Host` line and then the others. The `Resolved` keys are the
ones the picker's pane shows. Several matches exit 2 and name the candidates; no match exits 1;
a failing `ssh -G` exits 2 with ssh's own message.

## Scripting

    sshpick ls                  # every host, one per line
    sshpick ls new consul       # only the hosts matching the query
    sshpick ls | cut -f2        # just the host names
    sshpick --list | fzf        # the older spelling, still works

On a terminal `ls` prints an aligned table with a dim header, the way the picker draws its rows,
and a line on stderr such as `> 4 of 12 hosts match "consul"`. ENV is left out when no host has
one, and WHAT is cut with `…` to the width of the terminal.

When stdout is not a terminal `ls` prints the matching hosts as `ENV`, `HOST` and `WHAT`,
separated by tabs, with no header and no styling. The order is the picker's list read top to
bottom, best match first; with no query it is the order of the config. There are always three
columns, so `cut -f2` gives the host names even when ENV is empty. Tabs inside a comment become
spaces. Warnings go to stderr. The exit status is 1 when nothing matches.

`--list` and `-l` stay as hidden spellings of `ls` that always print the tab-separated rows,
even on a terminal, so `sshpick --list | fzf` keeps working.

`ls --json` and `inspect --json` print one JSON object, indented by two spaces, and nothing else
on stdout. Keys are fixed and an empty result still prints the object (and `ls` exits 1).

    $ sshpick ls consul --json
    {
      "query": "consul",
      "total": 12,
      "hosts": [
        {
          "host": "stg-consul1",
          "env": "STAGING",
          "what": "consul server 1",
          "names": ["stg-consul1", "consul1.stg"]
        }
      ],
      "warnings": []
    }

`env` and `what` are always there, as `""` when empty. A warning is `{"kind", "path", "message"}`
with `kind` one of `unreadable`, `too_deep`, `cycle` or `bad_include`; `path` is left out when
the warning has none. `inspect --json` is `{host, env, what, names, resolved, command}`, where
`resolved` has ssh's own keys (`user`, `hostname`, `port`, `remotecommand`, `proxyjump`) and
leaves out the ones ssh did not print, and `command` is the argv that `pick` would run.
Errors still go to stderr as text.

Only results go to stdout. Help and `--version` also go to stdout; warnings, errors and the `>`
line go to stderr. An error is an `Error:` line, then a `Fix:` line when there is a next step and
a `Learn More:` line when the docs help; a warning is a `! message` line:

    $ sshpick consul < /dev/null
    Error: 4 hosts match "consul", and the picker needs an interactive terminal
    Fix: narrow the query to one host, or list them with `sshpick ls consul`.

A near-miss gets one suggestion, only when it is a single edit from a command or option:
`sshpick help lst` says ``did you mean `sshpick help list`?``. A pipe gets no escape bytes unless `CLICOLOR_FORCE` is set. Colour is decided
per stream: `--no-color` or a non-empty `NO_COLOR` removes colour but keeps bold and dim on a
terminal, `CLICOLOR_FORCE` (set, not empty, not `0`) keeps colour in a pipe, and `TERM=dumb`
turns it off.

## Exit status

| Status      | Meaning                                                                     |
|-------------|-----------------------------------------------------------------------------|
| 0           | `ls`, `inspect`, `--help` or `--version` succeeded                          |
| `ssh`'s own | after a pick (sshpick becomes `ssh`)                                        |
| 1           | Enter with no match, or no match without the picker (`ls`, `inspect`)       |
| 2           | config error, no hosts found, unknown option or command, a failed `ssh -G`, |
|             | or several matches where one is needed                                      |
| 127         | `ssh` could not be run                                                      |
| 130         | cancelled                                                                   |

## Install

    make install            # cargo install --locked --path crates/sshpick --root ~/.local
    ln -sfn ~/.local/bin/sshpick ~/.local/bin/nipa-ssh

`~/.local/bin` must be on `PATH`. If `~/.local/bin/sshpick` is a symlink, `make install`
removes it first. It leaves a regular file alone, because cargo replaces its own binary.

## Migrating from the bash script

The binary replaces the fzf-based bash script that used to live at the repo root. If
`~/.local/bin/sshpick` (or a second name such as `nipa-ssh`) still points at that script:

1. `make install` replaces the symlink with the binary.
2. `ln -sfn ~/.local/bin/sshpick ~/.local/bin/nipa-ssh` repoints the second name.
3. `sshpick --version` and `nipa-ssh --version` should both print a version.

What differs from the script:

- It reads `~/.ssh/config` and its `Include`s, not only `config.d/*.conf`. Hosts in the root
  file get a blank ENV.
- `Host` lines follow ssh syntax. The script missed `host`, `Host=x`, tabs and indented lines,
  and printed `!negated` names as hosts.
- All words on the command line are used (`sshpick new consul`). The script used the first.
- CRLF files and a BOM are handled, and labels do not depend on the locale.
- Other names on a `Host` line are searchable.
- Columns are aligned and matches are highlighted. The preview shows ssh's error.

## Development

    make setup              # once per clone: enables the pre-push hook
    make quality            # fmt, taplo, clippy (-D warnings), cargo-deny, tests; CI runs the same
    make fix                # cargo fmt and taplo format

Needs Rust 1.99 (`rust-toolchain.toml`), and `taplo` + `cargo-deny` for `make quality`
(`brew install taplo cargo-deny`). Layout: `crates/sshpick-core` holds every rule and has no
terminal dependency; `crates/sshpick-ui` is the terminal and everything printed;
`crates/sshpick-cli` is argv, the commands and the exit codes; `crates/sshpick` is the binary, a
few lines. Tests read `crates/*/tests/fixtures`, never your `~/.ssh`. Help and frame tests are
insta snapshots; after a change run `INSTA_UPDATE=always cargo test` and read the `.snap` diffs.

## License

MIT
