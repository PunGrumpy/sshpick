# Scripting

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
          "names": [
            "stg-consul1",
            "consul1.stg"
          ]
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
line go to stderr. When stdout is a terminal, `> ssh -- host` is printed before ssh starts. An
error is an `Error:` line, then a `Fix:` line when there is a next step and a `Learn More:` line
when the docs help; a warning is a `! message` line:

    $ sshpick consul < /dev/null
    Error: 4 hosts match "consul", and the picker needs an interactive terminal
    Fix: narrow the query to one host, or list them with `sshpick ls consul`.

`TERM=dumb` keeps the picker closed too, because a dumb terminal cannot move the cursor: several
matches exit 2, none exit 1, and exactly one still connects.

A near-miss gets one suggestion, only when it is a single edit from a command or option:
`sshpick help lst` says ``did you mean `sshpick help list`?``.

A pipe gets no escape bytes unless `CLICOLOR_FORCE` is set. Colour is decided per stream, in this
order:

| Setting                                          | Effect                                       |
|--------------------------------------------------|----------------------------------------------|
| `--no-color`, or a non-empty `NO_COLOR`          | no colour; bold and dim stay on a terminal   |
| `CLICOLOR_FORCE` set, not empty, not `0`         | colour even when output is not a terminal    |
| `TERM=dumb`                                      | no colour                                    |
| none of these                                    | colour when the stream is a terminal         |

`COLUMNS` sets the width of help when stdout is not a terminal (80 if unset).

## Doctor

`sshpick doctor` checks the config, ssh and this terminal, and changes nothing. It reads the config,
runs `ssh -V` and one `ssh -G` for the first host, and prints a report on stdout. stderr stays
empty, also when a check fails; a terminal gets one `>` note only when a host is named `doctor`.

    $ sshpick doctor
    [doctor] ok=5 warn=1 fail=0
    [doctor] config=/home/me/.ssh/config
    [ok] config: read /home/me/.ssh/config and 2 included files
    [ok] hosts: 29 hosts, 24 with an ENV label, 17 with a WHAT comment
    [warn] include: /home/me/.ssh/config.d/9-late.conf: skipped, include cycle; remove the Include that points back
    [ok] ssh: OpenSSH_9.6p1 Ubuntu-3ubuntu13.5, OpenSSL 3.0.13 30 Jan 2024
    [ok] resolve: ssh -G answered for prod-web1 in 14 ms
    [ok] terminal: 120x30, TERM=xterm-256color, colour on (stdout is a terminal); the picker can open here

There is one line per check, `[ok]`, `[warn]` or `[fail]` and the check's name, so
`sshpick doctor | grep '^\[fail\]'` finds what broke. A check that is not `ok` ends with a next
step after `; `, with the command in backticks. `[doctor] config=` is left out when there is no
home directory. Only the bracketed token is coloured, yellow or red, and only when stdout is a
terminal with colour on.

A `warn` is something the picker survives: an `Include` that was skipped, or a terminal where the
picker would not open (`sshpick <query>` still connects when one host matches). A `fail` is
something that stops sshpick from working. The exit status is 2 when any check is `fail`, after the
report is printed, and 0 otherwise, so `sshpick doctor && sshpick` runs the picker only when the
setup is sound. `--non-interactive` is part of the terminal check: with it the picker is closed.

### `doctor --json`

`doctor --json` prints the same report as one JSON object, indented by two spaces, with a trailing
newline. `config` is left out when there is no home directory, and so is a check's `fix` when it
has none. `status` is `ok`, `warn` or `fail`; `summary` counts them.

    $ sshpick doctor --json
    {
      "config": "/home/me/.ssh/config",
      "summary": {
        "ok": 1,
        "warn": 1,
        "fail": 2
      },
      "checks": [
        {
          "name": "config",
          "status": "ok",
          "detail": "read /home/me/.ssh/config"
        },
        {
          "name": "hosts",
          "status": "fail",
          "detail": "no hosts found in the config or the files it Includes",
          "fix": "add `Host` lines; ENV and WHAT come from comments"
        },
        {
          "name": "ssh",
          "status": "fail",
          "detail": "not found on PATH",
          "fix": "install OpenSSH, or put `ssh` on your PATH"
        },
        {
          "name": "terminal",
          "status": "warn",
          "detail": "TERM=xterm-256color, colour off (stdout is not a terminal); the picker needs an interactive terminal",
          "fix": "`sshpick <query>` still connects when one host matches"
        }
      ]
    }

`name` is one of a fixed set, listed in the order a report has them. `name` and `status` are for
scripts; `detail` and `fix` are for people and may be reworded.

| Check      | Status                                                                              |
|------------|-------------------------------------------------------------------------------------|
| `config`   | `fail` when there is no home directory or the root config cannot be read            |
| `hosts`    | `fail` when no `Host` line names a host                                             |
| `include`  | one `warn` per config warning, left out when there are none                         |
| `ssh`      | `fail` when `ssh -V` cannot run                                                     |
| `resolve`  | `fail` when `ssh -G` fails for the first host; left out when there is no host       |
| `terminal` | `warn` when the picker would not open here, as `pick` would refuse it               |

## Exit status

| Status      | Meaning                                                                     |
|-------------|-----------------------------------------------------------------------------|
| 0           | `ls`, `inspect`, `doctor`, `--help` or `--version` succeeded                |
| `ssh`'s own | after a pick (sshpick becomes `ssh`)                                        |
| 1           | Enter with no match, or no match without the picker (`ls`, `inspect`)       |
| 2           | config error, no hosts found, unknown option or command, a failed `ssh -G`, |
|             | a failed `doctor` check, or several matches where one is needed             |
| 127         | `ssh` could not be run                                                      |
| 130         | cancelled                                                                   |
