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

To install sshpick and its short name `sp`, run this command:

```bash
cargo install --locked --git https://github.com/PunGrumpy/sshpick sshpick
```

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

## For coding agents

List the hosts with `sshpick ls --json`, see where one resolves with
`sshpick inspect <host> --json`, and check the setup with `sshpick doctor --json`. These three
change nothing and never prompt, so run them freely.

Only `sshpick <query>` connects to a host. It replaces itself with `ssh -- <host>`, so give it a
query that matches exactly one host, and add `--non-interactive` so it can never open the picker:
one match connects, none exits 1, several exit 2.

Results go to stdout and nothing else does. Errors are an `Error:` line and, when there is a next
step, a `Fix:` line on stderr. Exit codes: 0 success (after a pick, ssh's own), 1 no match, 2 usage,
config or ssh error or a failed `doctor` check, 127 ssh could not be run, 130 cancelled. The JSON
shapes are in [Scripting](docs/scripting.md).

Only run `sshpick` with no query when a person is at the keyboard: it opens a picker.

## Documentation

See the [documentation](docs/README.md) for the config conventions, every command and key, and the
output formats for scripts. When something does not work, start with `sshpick doctor` and
[Troubleshooting](docs/troubleshooting.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for more information.

## Security

If you believe you have found a security vulnerability in sshpick, please do not open a public
issue. Report it privately through
[GitHub security advisories](https://github.com/PunGrumpy/sshpick/security/advisories/new).

## License

[MIT](LICENSE)
