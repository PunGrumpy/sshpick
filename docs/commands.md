# Commands

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
