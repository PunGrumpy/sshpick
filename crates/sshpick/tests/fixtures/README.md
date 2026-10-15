Byte-exact inputs; `.gitattributes` and `.editorconfig` keep tools from rewriting them.

- `home/.ssh/config`: exactly one literal host, so `sshpick` with any query is a single match and
  `tests/pick_test.rs` sees it exec the fake `ssh` without a terminal.
- `ui/.ssh/`: three labelled hosts per environment file, for the `--list` tests. `sshpick-ui`
  keeps its own copy for the frame tests.
- `flat/.ssh/config`: hosts in the root file only, so the ENV column hides.
- `words/.ssh/config`: hosts named `list` and `web`, so `pick list` and `-- list` can be shown to
  reach a host whose name is also a command.
- `cycle/.ssh/config`: one host and an `Include` of itself, so the run has one warning to show.
- `inspect/.ssh/`: `stg-consul1` (alias `consul1.stg`, label STAGING), `stg-consul2`, `stg-broken`
  (the fake `ssh -G` fails for it) and a host named `list`, for `inspect` and `list --json`.
- `empty/.ssh/config`: no literal host (only `Host *`), so `sshpick` has nothing to pick.
- `tabs/.ssh/config`: a tab inside the WHAT comment, so `--list` must still print three columns.
- `bin/ssh`: stands in for `ssh` on `PATH`. `ssh -G -- host` answers like ssh (and fails for
  `stg-broken`); anything else prints how it was called.
