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
line go to stderr. An error is an `Error:` line, then a `Fix:` line when there is a next step and
a `Learn More:` line when the docs help; a warning is a `! message` line:

    $ sshpick consul < /dev/null
    Error: 4 hosts match "consul", and the picker needs an interactive terminal
    Fix: narrow the query to one host, or list them with `sshpick ls consul`.

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
