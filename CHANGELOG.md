# Changelog

All notable changes to sshpick. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/). The release workflow publishes
the top versioned section as the release notes.

## [Unreleased]

## [0.2.0] - 2026-11-08

The first tagged release. Everything before it was installed from `main`.

### Breaking

- `--non-interactive` connects only to an exact host name or alias; a lone fuzzy match now exits 1
  and names the host it would have guessed (#27).

### Added

- A `candidates` array in `several_matches` JSON errors (#29).
- A `timeout` reason when `ssh -G` gives no answer within 2 s; `doctor` reports it as `warn` (#28).
- `skills/sshpick/SKILL.md`, a guide for coding agents (#TBD).
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
