Byte-exact inputs; `.gitattributes` and `.editorconfig` keep tools from rewriting them.

- `home/.ssh/config`: exactly one literal host, so `sshpick` with any query is a single match and
  `tests/pick_test.rs` sees it exec the fake `ssh` without a terminal.
- `ui/.ssh/`: three labelled hosts per environment file, for the `--list` tests. `sshpick-ui`
  keeps its own copy for the frame tests.
- `flat/.ssh/config`: hosts in the root file only, so the ENV column hides.
- `words/.ssh/config`: hosts named `list` and `web`, so `pick list` and `-- list` can be shown to
  reach a host whose name is also a command.
- `cycle/.ssh/config`: one host and an `Include` of itself, so the run has one warning to show.
- `warns/.ssh/config`: two hosts and an `Include` of itself, so a run that does not connect shows
  one warning above its error, and the picker has something to open on.
- `inspect/.ssh/`: `stg-consul1` (alias `consul1.stg`, label STAGING), `stg-consul2`, `stg-broken`
  (the fake `ssh -G` fails for it) and a host named `list`, for `inspect` and `list --json`.
- `empty/.ssh/config`: no literal host (only `Host *`), so `sshpick` has nothing to pick.
- `tabs/.ssh/config`: a tab inside the WHAT comment, so `--list` must still print three columns.
- `named-doctor/.ssh/config`: a host named `doctor`, so `sshpick doctor` is the command and
  `sshpick pick doctor` is the host.
- `broken/.ssh/config`: one host, `stg-broken`, for which the fake `ssh -G` fails, so `doctor` has a
  failing `resolve` check.
- `bin/ssh`: stands in for `ssh` on `PATH`. `ssh -V` prints a version on stderr, as OpenSSH does.
  `ssh -G -- host` answers like ssh (and fails for `stg-broken`); anything else prints how it was
  called. When its stdin is a terminal it first prints `COOKED` or `RAW` on stderr, from
  `stty -a`, for `picker_test.rs`.
- `bin-read-only/ssh`: answers `-V` and `-G` like `bin/ssh` and exits 99 for anything else, so
  `read_only_test.rs` fails if a read-only command asks ssh to do more than read.
