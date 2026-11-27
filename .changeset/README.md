# Changesets

A user-facing change adds a changeset: run `bun run changeset`, pick `keyway`, choose the bump, and
write the entry as it should read in `crates/keyway/CHANGELOG.md`. The first line is the list
item; later lines are indented under it.

On `main`, the Release workflow keeps a `chore: Release keyway` pull request open that applies
the pending changesets: it bumps `crates/keyway/package.json`, mirrors the version into
`Cargo.toml`, `Cargo.lock` and `README.md` with `scripts/sync-version.ts`, and writes the
changelog. Merging it publishes the release. See [CONTRIBUTING.md](../CONTRIBUTING.md#releasing).

Docs, CI and refactors that change nothing a user sees need no changeset.
