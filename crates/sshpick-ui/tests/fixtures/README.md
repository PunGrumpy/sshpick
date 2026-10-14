Byte-exact inputs; `.gitattributes` and `.editorconfig` keep tools from rewriting them.

- `ui/.ssh/`: three labelled hosts per environment file, for the frame tests in `src/tui/`.
- `flat/.ssh/config`: hosts in the root file only, so the ENV column hides.
- `home/.ssh/config`: exactly one literal host, for the singular `1 host` count.
