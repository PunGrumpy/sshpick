# Troubleshooting

Start with `keyway doctor`. It reads the config, runs `ssh -V` and one `ssh -G`, looks at the
terminal it was started in, and prints one line per check: `[ok]`, `[warn]` or `[fail]`, the check's
name, what it found and, when there is a next step, the step after a `;`. It changes nothing, and
exits 2 when a check fails. [Scripting](scripting.md) has the exact format and the JSON twin,
`doctor --json`.

Below, by symptom, are the lines that show it and the fix. Run `keyway doctor` where the problem
is: in the same shell, pipe or editor terminal, because the `terminal` check describes the stdin
and stdout it was started with.

## The picker does not open

`keyway consul` prints an error instead, and exits 2 or 1:

```text
Error: 4 hosts match "consul", and the picker needs an interactive terminal
```

```text
[warn] terminal: TERM=xterm-256color, colour off (stdout is not a terminal); the picker needs an interactive terminal; `keyway <query>` still connects when one host matches
```

The picker opens only when stdin and stdout are both terminals, `--non-interactive` is not given
and `TERM` is not `dumb`. The `terminal` check names the first of these that fails:

- `the picker needs an interactive terminal`: stdin or stdout is a pipe or a file. Run `keyway`
  from a terminal, without `|` or `<` around it.
- `--non-interactive never opens the picker`: leave the flag out.
- `the picker needs a terminal that moves the cursor (TERM=dumb)`: set `TERM` to your terminal's
  type, such as `xterm-256color`.

When the picker is refused by the terminal, a query that matches exactly one host still connects,
none exits 1 and several exit 2. Under `--non-interactive` only an exact host name or alias
connects; a loose single match exits 1 and names the host it would have guessed.

## A host is missing

```text
[ok] hosts: 12 hosts, 8 with an ENV label, 5 with a WHAT comment
[warn] include: /home/me/.ssh/config.d/9-late.conf: skipped, include cycle; remove the Include that points back
```

Compare the count with the hosts you expect, and check `keyway ls <name>`. Then look for one of:

- An `Include` was skipped, which also skips the hosts in that file. Read the `include` lines
  below.
- The name is a pattern. `Host *.corp` and `Host !web` are not hosts, so they are skipped; list
  a literal name on the `Host` line to get a row.
- The file is hidden. Like ssh, an `Include` with a wildcard skips files that start with a dot.
- The name is listed twice. A name listed twice makes one row.

With no hosts at all the check fails:

```text
[fail] hosts: no hosts found in the config or the files it Includes; add `Host` lines; ENV and WHAT come from comments
```

## An Include is skipped

Each skipped `Include` is one `warn`, with the same text the other commands print as a `! …` line
on stderr. Only the file or pattern it names is lost; the rest of the config is still read.

```text
[warn] include: /home/me/.ssh/config.d/9-late.conf: skipped, include cycle; remove the Include that points back
[warn] include: /home/me/.ssh/config.d/old.conf: skipped, Permission denied (os error 13); make the file readable, or remove the Include that names it
[warn] include: Include hosts/%h.conf: skipped, %h is not supported; fix the Include pattern
[warn] include: /home/me/.ssh/chain/d17.conf: skipped, Include nested deeper than 16 levels; flatten the Include chain
```

A `%` token other than `%d` and `%%` depends on the host, so keyway cannot expand it. ssh itself
stops at 16 levels of `Include`.

## `ssh -G` fails or is slow

The route shows `ssh -G failed: …` instead of where the host goes, or `resolving…` for a long
time. `doctor` asks `ssh -G` about the first host and times it:

```text
[fail] resolve: ssh -G failed for stg-broken: /home/me/.ssh/config.d/10-staging.conf line 6: Bad configuration option: bogus; run `keyway inspect stg-broken` for ssh's full answer
[ok] resolve: ssh -G answered for lab-a in 14 ms
```

One bad option anywhere in the config makes `ssh -G` fail for every host, so one probe is enough
to see it. Run `keyway inspect <host>` for ssh's whole message, fix the line it names, and run
`keyway doctor` again.

A long time, or no answer at all, means ssh itself is slow to read the config:

```text
[warn] resolve: ssh -G gave no answer for prod-web1 within 2 s; the picker shows a route once ssh -G answers; time `ssh -G -- prod-web1` to find the slow `Match exec` or `ProxyCommand`
```

`ssh -G` runs any `Match exec` command in the config, and a `ProxyCommand` or `Match exec` that
waits on the network slows every host. Time `ssh -G -- <host>` yourself to confirm. keyway stops
waiting after 2 s and kills what ssh started. `keyway inspect <host>` then exits 2 with
`reason: timeout` under `--json`. The picker still connects, and a host whose answer timed out is
asked again the next time it is selected.

## Colours are wrong or missing

The parentheses in the `terminal` line name the rule that decided:

```text
[ok] terminal: 80x24, TERM=xterm-256color, colour off (NO_COLOR is set); the picker can open here
```

| It says                    | What to do                                                       |
|----------------------------|------------------------------------------------------------------|
| `--no-color`               | Leave the flag out                                               |
| `NO_COLOR is set`          | Unset `NO_COLOR`; an empty value is ignored                      |
| `CLICOLOR_FORCE is set`    | Colour is on even in a pipe; unset it to stop                    |
| `TERM=dumb`                | Set `TERM` to your terminal's type                               |
| `stdout is a terminal`     | Colour is on                                                     |
| `stdout is not a terminal` | Colour is off in a pipe; set `CLICOLOR_FORCE=1` to keep it       |

The rules are tried in this order, and [Scripting](scripting.md) lists them. Without colour, bold,
dim and reverse video stay on a terminal, except under `TERM=dumb`. The picker follows the same
rules for the terminal it draws on: with colour the selected host is a blue bar, and without it the
bar is reverse video, and the border is dim. Colour shows up in the picker, help, errors, warnings
and the `doctor` tokens.

## The picker has no border

The picker is a panel nearly the whole window. A terminal narrower than 60 columns or shorter
than 10 rows draws it without the border: the full width when it is narrow, with the same rows.
Make the window bigger to get the border back.

## The terminal looks wrong after keyway was killed

Killed by SIGINT, SIGTERM, SIGHUP or SIGQUIT, keyway leaves the alternate screen and puts the
terminal back before it ends. SIGKILL (`kill -9`) cannot be handled, and neither can a power cut,
so the terminal can stay on the picker's screen in raw mode, with a hidden cursor and no echo.

Nothing in `keyway doctor` shows this: it checks what keyway finds, not what a killed process
left behind. Its `terminal` line does name the `TERM` that `reset` will use:

```text
[ok] terminal: 80x24, TERM=xterm-256color, colour on (stdout is a terminal); the picker can open here
```

Type `reset` and press Enter, even if you cannot see what you type. `stty sane` restores echo and
line editing but not the cursor.

## keyway says ssh could not be run

`keyway consul` exits 127 with `Error: cannot run ssh: …`, and `keyway inspect` exits 2 with
`Error: cannot run ssh -G: …`:

```text
[fail] ssh: not found on PATH; install OpenSSH, or put `ssh` on your PATH
```

Install OpenSSH, or add the directory of your `ssh` to `PATH` in the shell that runs keyway. Any
other `ssh -V` failure shows as `ssh -V failed: …` with the reason, such as `exited with 255` for a
wrapper that rejects the flag.

## keyway cannot read the config

```text
[fail] config: cannot read /home/me/.ssh/config: No such file or directory (os error 2); create `~/.ssh/config`, or make it readable
[fail] config: cannot find the home directory; set `HOME`
```

keyway starts from `~/.ssh/config`. The `hosts`, `include` and `resolve` checks need it, so they
are left out of the report until it can be read. [Configuration](configuration.md) shows what a
config needs.

## Recent hosts do not come first

The picker lists hosts in the order of the config and has no LAST column. Look for one of:

- The picker did not open. Only a connection made at a terminal is remembered, so `--non-interactive`,
  a pipe and `TERM=dumb` record nothing. `ls` uses the history only with `--recent`.
- A `! cannot save recent hosts to …` line was printed before ssh started. The directory it names
  cannot be written. Make it writable, or point `XDG_STATE_HOME` at a directory you own.
- `XDG_STATE_HOME` is empty or relative. Only an absolute path counts. Otherwise the history is in
  `~/.local/state/keyway/history`.
- `history` is a link to `/dev/null`, which keeps nothing. Remove the link to start again.
- The host was renamed and its old name is no longer on the `Host` line.
- You typed a query. Last use only orders hosts that match equally well, so a better match stays
  above a host you used more recently.
