---
"keyway": minor
---

sshpick is now keyway, and its short name `sp` is now `kw`. Recent hosts live in
`~/.local/state/keyway`, and the Rex integration is `integrations/rex/keyway.lua`, loaded with
`require("keyway")`. See `docs/migrating.md` to move a machine over.
