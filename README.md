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
```

```text
> consul                                                                    4/12
  ENV     HOST          WHAT
▌ STAGING stg-consul1   consul server 1
  STAGING stg-consul2   consul server 2
  STAGING stg-consul3   consul server 3
  STAGING stg-consul-ui consul web UI tunnel: `ssh -f stg-consul-ui`, then open…




  → ubuntu@192.0.2.11:22 via stg-bastion
  ↑↓ move   enter ssh   esc quit   F1 keys
```

## Documentation

See the [documentation](docs/README.md) for the config conventions, every command and key, and the
output formats for scripts.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for more information.

## Security

If you believe you have found a security vulnerability in sshpick, please do not open a public
issue. Report it privately through
[GitHub security advisories](https://github.com/PunGrumpy/sshpick/security/advisories/new).

## License

[MIT](LICENSE)
