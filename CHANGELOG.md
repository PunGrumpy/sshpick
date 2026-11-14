# Changelog

All notable changes to sshpick. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/). The release workflow publishes
the top versioned section as the release notes.

## [Unreleased]

### Added

- `sshpick --print` prints the picked host instead of connecting, and `--print --json` prints its
  `{host, env, what, names}` object. The picker draws on `/dev/tty`, so `$(sshpick --print)`
  captures only the host.
- A host with no comment of its own takes the heading of its section as its WHAT: the first line
  of a comment block after a blank line, up to the next blank line or the next host with its own
  comment.

- The picker remembers the hosts you connect to at a terminal. It lists them first with an empty
  filter, breaks ties between equal matches by last use, and shows when in a dim LAST column.
  `XDG_STATE_HOME` moves the history from `~/.local/state/sshpick`.
- An address, part of one, a user or `user@address` from shell history finds a host, from the
  `HostName` and `User` written in its `Host` block, or the `user@target` of its `RemoteCommand`.

### Changed

- Each search word matches inside one column. `git`, `ru` or `db` no longer find hosts by letters
  spread across ENV, HOST and WHAT, so look-alike hosts in other environments drop out. A word no
  host contains still matches when each letter starts a word: `pdb` finds `prod-db-primary`.

### Fixed

- `^` and `$` anchor the start and end of each column, so `^prod-db` and `prod$` find hosts, and a
  capital `^PROD` or `API$` is no longer always empty.

### Fixed

- An ENV label line such as `# PROD: production, VPN only` shows `PROD`. Before, the whole line
  filled the ENV column and pushed WHAT off the screen.

## [0.2.0] - 2026-11-08

The first tagged release. Everything before it was installed from `main`.

### Breaking

- `--non-interactive` connects only to an exact host name or alias; a lone fuzzy match now exits 1
  and names the host it would have guessed (#27).

### Added

- A `candidates` array in `several_matches` JSON errors (#29).
- A `timeout` reason when `ssh -G` gives no answer within 2 s; `doctor` reports it as `warn` (#28).
- `skills/sshpick/SKILL.md`, a guide for coding agents (#31).
- Prebuilt binaries for Linux and macOS on each release, and `rust-version` in the manifest.

### Changed

- `--version` works after any command, and help is shorter (#23).
- Under `--json`, a failure is one JSON line on stderr with a stable `reason` (#24).

### Fixed

- A query equal to one host's name selects that host even when it also matches others (#27).
- `ssh -G` returns within its 2 s bound and kills what it started (#28).
- Error text is safe to print and to paste: control characters become spaces, queries in `Fix:`
  are shell-quoted, and a closed stderr no longer panics (#26).

[Unreleased]: https://github.com/PunGrumpy/sshpick/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/PunGrumpy/sshpick/releases/tag/v0.2.0
