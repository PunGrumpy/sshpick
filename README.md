<p align="center">
  <img src="assets/sshpick-icon.svg" width="128" height="128" alt="">
</p>

<p align="center">
  <h1 align="center">sshpick</h1>
</p>

<p align="center">
  <a aria-label="CI status" href="https://github.com/PunGrumpy/sshpick/actions/workflows/ci.yml"><img alt="" src="https://img.shields.io/github/actions/workflow/status/PunGrumpy/sshpick/ci.yml?branch=main&style=for-the-badge&labelColor=000000&label=CI"></a>
  <a aria-label="Rust version" href="https://github.com/PunGrumpy/sshpick/blob/main/rust-toolchain.toml"><img alt="" src="https://img.shields.io/badge/Rust-1.99-000000.svg?style=for-the-badge&logo=rust&labelColor=000000"></a>
  <a aria-label="License" href="https://github.com/PunGrumpy/sshpick/blob/main/LICENSE"><img alt="" src="https://img.shields.io/github/license/PunGrumpy/sshpick.svg?style=for-the-badge&labelColor=000000&color="></a>
</p>

sshpick is a fuzzy picker for the hosts in your ssh config, with a live `ssh -G` preview of where
each host really goes. It runs on macOS and Linux.

## Getting Started

Download the archive for your platform from the
[latest release](https://github.com/PunGrumpy/sshpick/releases/latest), check it against the
`.sha256` file beside it, and put `sshpick` and its short name `sp` on your `PATH`. Or build it with
cargo (Rust 1.99 or newer):

```bash
cargo install --locked --git https://github.com/PunGrumpy/sshpick --tag v0.2.0 sshpick
```

`sshpick --version` prints the version you have. To upgrade, download the new archive, or run the
install command again with the new tag and `--force`.

Then pick a host:

```bash
sshpick                       # every host in ~/.ssh/config and its Includes
sshpick consul                # pre-filtered; exactly one match connects at once
sshpick ls consul             # print the matching hosts instead
sshpick inspect stg-consul1   # where a host resolves, without connecting
sshpick doctor                # check the config, ssh and this terminal
```

```text
~/work $ sshpick
> type to filter                                                             5/5
  ENV         HOST           WHAT
▌ LAB         lab-a          LAB — personal VMs
  LAB         lab-b          build box
  NEW STAGING newstg-consul1 consul server 1 — mgmt
  NEW STAGING newstg-consul2
  NEW STAGING newstg-bastion bastion



  → ubuntu@10.11.212.191:22 via newstg-bastion
  ↑↓ move   enter ssh   esc quit   ? keys
```

The picker remembers the hosts you connect to. They come first, with a dim LAST column that says
how long ago, so the host you used last is one Enter away.

## For coding agents

List the hosts with `sshpick ls --json`, see where one resolves with
`sshpick inspect <host> --json`, and check the setup with `sshpick doctor --json`. These three
change nothing and never prompt, so run them freely.

Take the exact host name or alias from `ls --json`, never from memory. Under `--non-interactive`
an exact name connects, a loose match or no match exits 1, and several exit 2.

`sshpick <query> --non-interactive` replaces itself with an interactive `ssh -- <host>`, which
needs a terminal. Without one, read `.host` from `sshpick inspect <host> --json` and run `ssh`
yourself with the remote command.

Results go to stdout and nothing else does. Errors are an `Error:` line and, when there is a next
step, a `Fix:` line on stderr. Under `--json` a failure is one JSON line on stderr instead, with a
stable `reason`, and `candidates` when several hosts match. `pick` takes `--json` only with
`--print`, which prints the picked host's `{host, env, what, names}` instead of connecting.

Exit codes: 0 success (after a pick, ssh's own), 1 no match, 2 usage, config or ssh error
or a failed `doctor` check, 127 ssh could not be run, 130 cancelled. The JSON shapes are in
[Scripting](docs/scripting.md).

The full guide for agents is [`skills/sshpick/SKILL.md`](skills/sshpick/SKILL.md). Copy or symlink
that directory into your agent's skills folder, for example `~/.claude/skills/`.

Only run `sshpick` with no query when a person is at the keyboard: it opens a picker.

## Documentation

See the [documentation](docs/README.md) for the config conventions, every command and key, and the
output formats for scripts. When something does not work, start with `sshpick doctor` and
[Troubleshooting](docs/troubleshooting.md). To pick hosts from a key in the Rex terminal, see
[Rex](docs/rex.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for more information.

## Security

If you believe you have found a security vulnerability in sshpick, please do not open a public
issue. Report it privately through
[GitHub security advisories](https://github.com/PunGrumpy/sshpick/security/advisories/new).

## License

[MIT](LICENSE)
