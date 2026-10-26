# Troubleshooting

Start with `sshpick doctor`. It reads the config, runs `ssh -V` and one `ssh -G`, looks at the
terminal it was started in, and prints one line per check: `[ok]`, `[warn]` or `[fail]`, the check's
name, what it found and, when there is a next step, the step after a `;`. It changes nothing, and
exits 2 when a check fails. [Scripting](scripting.md) has the exact format and the JSON twin,
`doctor --json`.

Below, by symptom, are the lines that show it and the fix. Run `sshpick doctor` where the problem
is: in the same shell, pipe or editor terminal, because the `terminal` check describes the stdin
and stdout it was started with.

## The picker does not open

`sshpick consul` prints an error instead, and exits 2 or 1:

```text
Error: 4 hosts match "consul", and the picker needs an interactive terminal
```

```text
[warn] terminal: TERM=xterm-256color, colour off (stdout is not a terminal); the picker needs an interactive terminal; `sshpick <query>` still connects when one host matches
```

The picker opens only when stdin and stdout are both terminals, `--non-interactive` is not given
and `TERM` is not `dumb`. The `terminal` check names the first of these that fails:

- `the picker needs an interactive terminal`: stdin or stdout is a pipe or a file. Run `sshpick`
  from a terminal, without `|` or `<` around it.
- `--non-interactive never opens the picker`: leave the flag out.
- `the picker needs a terminal that moves the cursor (TERM=dumb)`: set `TERM` to your terminal's
  type, such as `xterm-256color`.

When the picker is refused, a query that matches exactly one host still connects, none exits 1 and
several exit 2.

## A host is missing

```text
[ok] hosts: 12 hosts, 8 with an ENV label, 5 with a WHAT comment
[warn] include: /home/me/.ssh/config.d/9-late.conf: skipped, include cycle; remove the Include that points back
```

Compare the count with the hosts you expect, and check `sshpick ls <name>`. Then look for one of:

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

A `%` token other than `%d` and `%%` depends on the host, so sshpick cannot expand it. ssh itself
stops at 16 levels of `Include`.

## `ssh -G` fails or is slow

The route shows `ssh -G failed: …` instead of where the host goes, or `resolving…` for a long
time. `doctor` asks `ssh -G` about the first host and times it:

```text
[fail] resolve: ssh -G failed for stg-broken: /home/me/.ssh/config.d/10-staging.conf line 6: Bad configuration option: bogus; run `sshpick inspect stg-broken` for ssh's full answer
[ok] resolve: ssh -G answered for lab-a in 14 ms
```

One bad option anywhere in the config makes `ssh -G` fail for every host, so one probe is enough
to see it. Run `sshpick inspect <host>` for ssh's whole message, fix the line it names, and run
`sshpick doctor` again.

A time that is long, or `ssh -G gave no answer within 2 s`, means ssh itself is slow to read the
config: `ssh -G` runs any `Match exec` command in it, so a command that waits on the network slows
every host. Time `ssh -G -- <host>` yourself to confirm. A host whose answer timed out is asked
again the next time it is selected.

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
dim and reverse video stay on a terminal, except under `TERM=dumb`. The picker itself draws no
colour at all, so `NO_COLOR` changes nothing there. Colour shows up in help, errors, warnings and
the `doctor` tokens.

## The terminal looks wrong after sshpick was killed

Killed by SIGINT, SIGTERM, SIGHUP or SIGQUIT, sshpick erases the picker and puts the terminal back
before it ends. SIGKILL (`kill -9`) cannot be handled, and neither can a power cut, so the terminal
can stay in raw mode with a hidden cursor and no echo.

Nothing in `sshpick doctor` shows this: it checks what sshpick finds, not what a killed process
left behind. Its `terminal` line does name the `TERM` that `reset` will use:

```text
[ok] terminal: 80x24, TERM=xterm-256color, colour on (stdout is a terminal); the picker can open here
```

Type `reset` and press Enter, even if you cannot see what you type. `stty sane` restores echo and
line editing but not the cursor.

## sshpick says ssh could not be run

`sshpick consul` exits 127 with `Error: cannot run ssh: …`, and `sshpick inspect` exits 2 with
`Error: cannot run ssh -G: …`:

```text
[fail] ssh: not found on PATH; install OpenSSH, or put `ssh` on your PATH
```

Install OpenSSH, or add the directory of your `ssh` to `PATH` in the shell that runs sshpick. Any
other `ssh -V` failure shows as `ssh -V failed: …` with the reason, such as `exited with 255` for a
wrapper that rejects the flag.

## sshpick cannot read the config

```text
[fail] config: cannot read /home/me/.ssh/config: No such file or directory (os error 2); create `~/.ssh/config`, or make it readable
[fail] config: cannot find the home directory; set `HOME`
```

sshpick starts from `~/.ssh/config`. The `hosts`, `include` and `resolve` checks need it, so they
are left out of the report until it can be read. [Configuration](configuration.md) shows what a
config needs.
