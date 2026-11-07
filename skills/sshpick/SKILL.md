---
name: sshpick
description: Find ssh hosts from ~/.ssh/config and where they resolve, without opening a picker or a session. Use when a task names an ssh host, a bastion, a jump host, or asks which machine a name points to.
---

# sshpick

sshpick lists the hosts in `~/.ssh/config` and the files it includes, and shows where each one
resolves according to `ssh -G`. `sshpick ls`, `sshpick inspect` and `sshpick doctor` only read.
`sshpick <query>` is the one command that connects.

## When to use

- The task names an ssh host, a bastion or a jump host.
- You need to know which machine a name points to: its user, address, port and jump host.
- A person asks you to reach "the staging consul box" and you need its real host name.

Do not use it to open an interactive session yourself. See Connecting.

## Prerequisites

- `sshpick` is on `PATH`. `sshpick --version` prints its version.
- `sshpick doctor --json` exits 0 when no check fails and 2 when one does. A failed check has
  `status` `fail` in `checks` and a `fix`.
- Without a terminal the `terminal` check is `warn`. That is expected for an agent and does not
  change the exit status.

## Commands

```bash
sshpick ls --json                      # every host: host, env, what, names
sshpick ls consul --json               # hosts matching a query, best first
sshpick inspect stg-consul1 --json     # where one host resolves, and the ssh argv
sshpick doctor --json                  # config, ssh and terminal checks
sshpick --non-interactive stg-consul1  # connect: replaces itself with ssh -- stg-consul1
```

The first four change nothing and never prompt. Run them freely.

## Choosing a host

A query is fuzzy. Its words must all match, and they are matched against the ENV label, the host
name, the WHAT comment and the other names on the `Host` line.

A query equal to one host's name or alias selects that host, even when other hosts match too.
Under `--non-interactive`, any other query that leaves one match exits 1, and `Fix:` names the host
it would have picked:

```text
Error: no host is named "consul1"; "stg-consul1" matches loosely
Fix: connect with `sshpick --non-interactive stg-consul1`, or list candidates with `sshpick ls consul1`.
```

Take names from `sshpick ls --json`, as `.hosts[].host` or one of `.hosts[].names`. Never take them
from memory.

Never run bare `sshpick`, or a query without `--non-interactive`. On a terminal either one can open
the picker and wait for a person.

## Connecting

`sshpick <name> --non-interactive` replaces itself with an interactive `ssh -- <name>`, which needs
a terminal. Without one, resolve the host and run `ssh` yourself:

```bash
host=$(sshpick inspect stg-consul1 --json | jq -r .host)
ssh -o BatchMode=yes -o ConnectTimeout=10 -- "$host" uptime
```

`.command` is the exact argv sshpick would run, such as `["ssh", "--", "stg-consul1"]`. Append the
remote command after it. `BatchMode=yes` makes `ssh` fail instead of prompting for a password.
`ConnectTimeout=10` gives up on a host that does not answer.

## Structured output

`--json` works on `ls`, `inspect` and `doctor`. Each prints one JSON object, indented by two
spaces, on stdout and nothing else. stderr stays empty when the command succeeds.

`pick`, the command that connects, has no `--json`. Adding `--json` to it is a usage error that
exits 2. Its failures are `Error:` and `Fix:` lines on stderr, with the exit codes below.

- `ls`: `query`, `total`, `hosts` and `warnings`. Each entry of `hosts` has `host`, `env`, `what`
  and `names`. `total` counts every host in the config, not the matches. `env` and `what` are `""`
  when empty.
- `inspect`: `host`, `env`, `what`, `names`, `resolved`, `command` and `warnings`. `resolved` has
  ssh's own keys `user`, `hostname`, `port`, `remotecommand` and `proxyjump`, and leaves out any
  that ssh did not print. `port` is a string, such as `"22"`.
- `doctor`: `config`, `summary` and `checks`. `summary` counts `ok`, `warn` and `fail`. Each entry
  of `checks` has `name`, `status`, `detail` and, when there is a next step, `fix`.

A warning is `{"kind", "path", "message"}`. Script on `name`, `status`, `kind` and `reason`.
`message`, `detail` and `fix` are for people and may be reworded. The full shapes are in
[Scripting](https://github.com/PunGrumpy/sshpick/blob/main/docs/scripting.md).

## Errors

Under `--json`, a failure prints nothing on stdout and one JSON line on stderr:

```json
{"status":"action_required","reason":"several_matches","message":"2 hosts match \"consul\"; inspect needs exactly one","fix":"name one of stg-consul1, stg-consul2, or list them with `sshpick ls consul`.","candidates":["stg-consul1","stg-consul2"]}
```

`status`, `reason` and `message` are always there. `fix`, `warnings` and `candidates` are left out
when there is nothing to say, and `candidates` only comes with `several_matches`. The exit status
is the same as without `--json`. `reason` is stable, so act on it:

| `reason` | What to do |
|---|---|
| `usage` | Fix the command line. `fix` names the accepted options. |
| `no_match` | No host matches. Widen the query or run `sshpick ls --json`. |
| `several_matches` | Pick one of `candidates`, best first, and retry with that exact name. |
| `no_hosts` | The config has no `Host` lines. Stop and tell the person. |
| `config_error` | The config cannot be read, or there is no home directory. Run `sshpick doctor --json`. |
| `ssh_failed` | `ssh` could not be run, or `ssh -G` failed with an error. Run `sshpick doctor --json`. |
| `timeout` | `ssh -G` gave no answer within 2 s, often a slow `Match exec` or `ProxyCommand`. Retry once, then report. |
| `write_failed` | stdout or the terminal could not be written. Nothing to fix in sshpick. |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success. After a connect, `ssh`'s own status. |
| `1` | No host matches, or under `--non-interactive` only a loose one does. |
| `2` | A usage, config or ssh error, a failed `doctor` check, or several matches where one is needed. |
| `127` | `ssh` could not be run. |
| `130` | Cancelled. |

## Pitfalls

- `Host *` and other patterns, such as `Host *.corp` or `Host !web`, are not hosts. They are not
  listed and cannot be selected.
- A query also matches the ENV label and the WHAT comment, so `ls` can return hosts whose names do
  not contain your words. Select by exact name.
- `inspect` takes the one host a query matches, exact or not: `sshpick inspect consul1 --json`
  resolves `stg-consul1`. Check `.host` before you use it.
- Under `--json`, warnings such as a skipped `Include` are in `warnings`, not on stderr.
- `ls --json` exits 1 with an empty `hosts` array when nothing matches. That is not an error
  document.
