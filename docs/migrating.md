# Migrating from the bash script

The binary replaces the fzf-based bash script that used to live at the repo root. If
`~/.local/bin/sshpick` still points at that script:

1. `make install` replaces the symlink with the binary and adds `sp`.
2. Remove any other name you linked to the script.
3. `sshpick --version` and `sp --version` should both print a version.

What differs from the script:

- It reads `~/.ssh/config` and its `Include`s, not only `config.d/*.conf`. Hosts in the root
  file get a blank ENV.
- `Host` lines follow ssh syntax. The script missed `host`, `Host=x`, tabs and indented lines,
  and printed `!negated` names as hosts.
- All words on the command line are used (`sshpick new consul`). The script used the first.
- CRLF files and a BOM are handled, and labels do not depend on the locale.
- Other names on a `Host` line are searchable.
- Columns are aligned and matches are highlighted. The preview shows ssh's error.

`make install` removes a symlink at `~/.local/bin/sshpick` before it installs. It leaves a regular
file alone, because cargo replaces its own binary.
