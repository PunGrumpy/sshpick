Byte-exact inputs; `.gitattributes` and `.editorconfig` keep tools from rewriting them.

- `home/.ssh/config`: exactly one literal host, so `sshpick` with any query is a single match and
  `tests/cli.rs` sees it exec the fake `ssh` without a terminal.
- `ui/.ssh/`: three labelled hosts per environment file, for the `--list` tests. `sshpick-ui`
  keeps its own copy for the frame tests.
- `flat/.ssh/config`: hosts in the root file only, so the ENV column hides.
- `empty/.ssh/config`: no literal host (only `Host *`), so `sshpick` has nothing to pick.
- `tabs/.ssh/config`: a tab inside the WHAT comment, so `--list` must still print three columns.
- `bin/ssh`: stands in for `ssh` on `PATH` and prints how it was called.
