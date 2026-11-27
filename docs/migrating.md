# Migrating

## From sshpick

keyway was called sshpick, with the short name `sp`, before its first release. Nothing about how
it reads your config changed. To move a machine over:

1. `make install` puts `keyway` and `kw` in `~/.local/bin`. Delete `~/.local/bin/sshpick` and
   `~/.local/bin/sp`.
2. Move the recent hosts: `mv ~/.local/state/sshpick ~/.local/state/keyway` (under
   `$XDG_STATE_HOME` if you set it).
3. In Rex's `init.lua`, `require("keyway")` and bind `"keyway"` in place of `"sshpick"`, and link
   `integrations/rex/keyway.lua` where `sshpick.lua` was.
4. `keyway --version` and `kw --version` should both print a version.

## From the bash script

The binary replaces the fzf-based bash script that used to live at the repo root. If
`~/.local/bin/sshpick` still points at that script:

1. `make install` installs `keyway` and `kw`. Delete the symlink.
2. Remove any other name you linked to the script.
3. `keyway --version` and `kw --version` should both print a version.

What differs from the script:

- It reads `~/.ssh/config` and its `Include`s, not only `config.d/*.conf`. Hosts in the root
  file get a blank ENV.
- `Host` lines follow ssh syntax. The script missed `host`, `Host=x`, tabs and indented lines,
  and printed `!negated` names as hosts.
- All words on the command line are used (`keyway new consul`). The script used the first.
- CRLF files and a BOM are handled, and labels do not depend on the locale.
- Other names on a `Host` line are searchable.
- Columns are aligned and matches are highlighted. The route shows ssh's error.

`make install` removes a symlink at `~/.local/bin/keyway` before it installs. It leaves a regular
file alone, because cargo replaces its own binary.
