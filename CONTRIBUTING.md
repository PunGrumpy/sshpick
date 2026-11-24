Thank you for your interest in contributing to sshpick!

- [General dependencies](#general-dependencies)
- [Structure of the repository](#structure-of-the-repository)
- [Building sshpick](#building-sshpick)
- [Running tests](#running-tests)
- [Opening a pull request](#opening-a-pull-request)

## General dependencies

You will need these installed to work on this repository:

- [Rust](https://www.rust-lang.org/tools/install) 1.99. rustup picks it up from
  [`rust-toolchain.toml`](rust-toolchain.toml).
- [taplo](https://taplo.tamasfe.dev) and [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
  for `make quality`. On macOS: `brew install taplo cargo-deny`.
- [Bun](https://bun.com) 1.4.5, the version `package.json` names, for Changesets, Turborepo, the
  scripts in `scripts` and the website. On macOS: `brew install oven-sh/bun/bun`, then
  `bun install`.

## Structure of the repository

- `crates/sshpick-core` holds every rule: the config reader, ranking, the picker state and
  `ssh -G`. It cannot depend on a terminal crate, and `cargo deny` enforces that.
- `crates/sshpick-ui` is the terminal and everything sshpick prints.
- `crates/sshpick-cli` is argv, help, errors, exit codes and one file per command.
- `crates/sshpick` is the `sshpick` and `sp` binaries, a few lines each, and the tests that run
  them.
- `docs` is the user documentation.
- `apps/web` is the website, a Next.js static export built from `README.md` and `docs`.
- `scripts/sync-version.ts` copies the version from `crates/sshpick/package.json` into
  `Cargo.toml`, `Cargo.lock` and `README.md`.

The repository is also a [Turborepo](https://turborepo.dev) and Bun workspace:
`bunx turbo run build lint test` runs the Rust tasks through `crates/sshpick/package.json` and
builds the website. `bun run check` lints and checks the formatting of the website, `scripts` and
the root JS/TS/JSON config with [Ultracite](https://www.ultracite.ai); `bun run fix` fixes what
it can.

Each crate has a `README.md` with its own layout. [AGENTS.md](AGENTS.md) has the rules for where
new code goes.

## Building sshpick

```bash
make setup     # once per clone: a pre-push hook that runs cargo fmt and taplo
make install   # builds and installs sshpick and sp to ~/.local/bin
make fix       # cargo fmt and taplo format
```

## Running tests

```bash
make quality   # fmt, taplo, clippy -D warnings, cargo-deny and every test; CI runs the same
```

The pre-push hook does not run clippy, cargo-deny or the tests. Run `make quality` before pushing.

Tests read `crates/*/tests/fixtures`, never your `~/.ssh`, and put a fake `ssh` first on `PATH`.
Help text, usage errors and rendered frames are [insta](https://insta.rs) snapshots. After a
change, run `INSTA_UPDATE=always cargo test` and read the `.snap` diffs before you commit.

## Releasing

The version lives in `crates/sshpick/package.json`, and [Changesets](https://changesets.dev)
writes it.

1. A pull request that changes what users see runs `bun run changeset`, picks `sshpick` and the
   bump, and commits the `.changeset/*.md` file it writes. Its text becomes the changelog entry.
   Docs, CI and refactors need none.
2. On `main`, the Release workflow keeps a `chore: Release sshpick` pull request open. It runs
   `bun run version-packages`, which bumps `crates/sshpick/package.json`, writes
   `crates/sshpick/CHANGELOG.md` and mirrors the version into `Cargo.toml`, `Cargo.lock` and the
   install line in `README.md` with `scripts/sync-version.ts`.
3. Merge that pull request. The Release workflow sees no tag for the new version, builds Linux and
   macOS binaries, and publishes a GitHub release and its `vX.Y.Z` tag with that changelog
   section as its notes.

`make test` fails when `package.json` and `Cargo.toml` disagree.

## Opening a pull request

Fill in the pull request template. Titles follow
[Conventional Commits](https://www.conventionalcommits.org/) without a scope, and the subject
starts with a capital letter:

```
feat: Add Ctrl-W to delete the last word
fix: Keep the ENV label when line 1 has a BOM
```

A change to a command name, flag, exit code, environment variable or TSV column needs a test and
a line in `docs`.
