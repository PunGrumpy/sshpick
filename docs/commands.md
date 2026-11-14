# Commands

| Command                  | What it does                                                      |
|--------------------------|-------------------------------------------------------------------|
| `pick [query]...`        | The default. Connect when one host matches, else open the picker  |
| `list`, `ls [query]...`  | Print the hosts that match, best first                            |
| `inspect <query>...`     | Show where the one matching host resolves, as `ssh -G` sees it    |
| `doctor`                 | Check the config, ssh and this terminal; changes nothing          |
| `help [command]`         | Show help for a command; `sshpick help ls` is `sshpick ls --help` |

`pick`, `list`, `ls`, `inspect`, `doctor` and `help` are command words when they come first. Any other
first word is query text, so `sshpick consul` still searches. A host named exactly like a command
word is reached with `sshpick pick list`, `sshpick pick doctor` or `sshpick -- list`; when `list`,
`inspect` or `doctor` is also a host or an alias, a terminal gets a note saying so. `doctor` takes
no query: `sshpick doctor extra` is a usage error, and `help` takes one command, so
`sshpick help list extra` is too. `-h` and `--help` work anywhere and win over every other error.
`--version` and `-V` work anywhere, after a command too, and lose only to `--help`; `-v` is the
same flag.

Options go anywhere before a bare `--`:

| Option              | Effect                                                                 |
|---------------------|------------------------------------------------------------------------|
| `-h`, `--help`      | Show help; wins over every other error                                 |
| `-V`, `--version`   | Print the version and exit 0; loses only to `--help`                   |
| `--no-color`        | Same as `NO_COLOR=1`                                                   |
| `--json`            | Print one JSON document; `ls`, `inspect` and `doctor` take it          |
| `--non-interactive` | Never open the picker: an exact host name or alias connects, a loose single match exits 1, none exits 1, several exit 2 |

## Search

Words are AND terms and use fzf's extended syntax with smart case (a capital makes that word
case-sensitive). Each word must match inside one field of the host: ENV, HOST, WHAT, or one of the
other names on the `Host` line. A word never takes letters from two fields, so `git` does not find
`NEW STAGING newstg-bastion`.

| Term      | Matches                                                     |
|-----------|-------------------------------------------------------------|
| `consul`  | substring; word starts when no host contains it (see below) |
| `'consul` | substring                                                   |
| `^stg`    | the start of a field: `^prod-db` is a host, `^PROD` an ENV  |
| `1$`      | the end of a field: `prod$` finds `api-prod`                |
| `!ui`     | hosts with no field that contains `ui`                      |

A word with no operator matches as a substring when some host has it in a field. When none does,
its letters may skip, but each must start a word (the start of the field, or after a character
that is not a letter or digit, such as `-` or `.`) or follow the letter before it: `pdb` finds
`prod-db-primary` and not `prod-bastion`.

The `HostName` and `User` written in a host's `Host` block are searchable too, so an address, part
of one, or a user finds the host. So is the target of a `RemoteCommand` that hops on, such as
`10.0.0.5` in `sudo ssh -t root@10.0.0.5`. `ubuntu@10.13.1.9` from shell history means
`10.13.1.9`: the user is how you log in, not which host, and is usually inherited from `Host *`.
A word looks in these values only when no ENV, HOST, WHAT or alias of any
host matches it, so the `1` in `consul 1` means `consul1`, not every address with a 1 in it. These
fields are never highlighted. [Configuration](configuration.md#what-it-searches) says which values
count.

Hosts that match equally well are listed with the one you connected to last first. `ls` does not
do this.

A query equal to one host's name or alias selects that host, even when it also matches other rows.
Any other query that leaves one row connects on a terminal, and exits 1 under `--non-interactive`.

There is no `a | b` OR.

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

`names` is the first name on the `Host` line and then the others. The `Resolved` rows are the
ones the picker's route panel shows from 100 columns. Several matches exit 2 and name the candidates; no match exits 1;
a failing `ssh -G` exits 2 with ssh's own message.
