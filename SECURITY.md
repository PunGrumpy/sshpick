# Security

## Reporting a vulnerability

Report it privately through
[GitHub's vulnerability reporting](https://github.com/PunGrumpy/keyway/security/advisories/new).
Do not open a public issue or pull request for it.

Say what an attacker controls (a line in `~/.ssh/config`, a query, a file under
`~/.local/state/keyway`, Rex's input), what they gain, and the steps or a config that shows it.
You will get a reply within 7 days, and a fix or a decision within 30.

## Supported versions

Only the latest release gets fixes. A fix ships as a new release, with an advisory that names the
versions it affects.

## Scope

keyway reads `~/.ssh/config` and the files it includes, runs `ssh` with a host from that config,
and keeps a history of the hosts you connected to. These are in scope:

- Running a command, or passing an option to `ssh`, that the config or the query did not ask for.
- Text from a config, a host name or a query that reaches the terminal as an escape sequence, or
  reaches a `Fix:` line in a form that runs something else when pasted.
- Reading or writing files outside the config, its `Include`s and keyway's state directory.
- The Rex integration in `integrations/rex` running something other than keyway and Rex.

A config you wrote that runs a command through ssh's own options, such as `ProxyCommand`,
`LocalCommand` or `RemoteCommand`, is ssh doing what the config says, and is out of scope.
