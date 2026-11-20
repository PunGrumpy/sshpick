# sshpick documentation

- [Configuration](configuration.md): where the ENV, HOST, WHAT and LAST columns come from, the two
  comment conventions that fill them, and where recent hosts are kept.
- [Commands](commands.md): `pick`, `ls`, `inspect`, `doctor`, `help`, the global options and the
  search syntax.
- [Picker](picker.md): the keys and what each part of the screen means.
- [Scripting](scripting.md): tab-separated and JSON output, the `doctor` report, errors, colour and
  exit codes.
- [Rex](rex.md): a key in the Rex terminal that lists the hosts in Rex's command palette and opens
  the picked one in a labelled tab.
- [Troubleshooting](troubleshooting.md): by symptom, with the `sshpick doctor` line that shows it and
  the fix.
- [Agent skill](../skills/sshpick/SKILL.md): the guide a coding agent loads to use sshpick without
  a terminal.
- [Migrating from the bash script](migrating.md): for anyone who used the fzf script this replaced.
