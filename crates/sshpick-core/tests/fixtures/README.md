Byte-exact inputs; `.gitattributes` and `.editorconfig` keep tools from rewriting them.

- `home/.ssh/`: a synthetic config shaped like the real one (never copied from `~/.ssh`).
  `config.d/40-crlf.conf` is CRLF with a BOM; `9-late.conf` sorts after `40-crlf.conf` and
  includes itself. Expected rows are in `tests/load.rs`.
- `deep/.ssh/`: `config` includes `chain/d01.conf`, which includes `d02.conf`, and so on to
  `d17.conf`. ssh reads `d01` to `d16` and is fatal at `d17`; sshpick lists `h01` to `h16` and warns.
- `bin/`: fake `ssh` programs for `ssh::resolve` and `Previews` tests.
