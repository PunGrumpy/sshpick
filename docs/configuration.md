# Configuration

## What it shows

| Column | Where it comes from |
|--------|---------------------|
| ENV    | line 1 of each included file: `# OLD STAGING — anything` shows `OLD STAGING`; no label: the file name (`30-prod.conf` shows `prod`); hosts in `~/.ssh/config` itself: blank (the column hides when all are) |
| HOST   | the first name on the `Host` line, the one you would type; `*`/`?` patterns and `!negations` are skipped; the other names are searchable but not shown |
| WHAT   | the `#` comment on the line directly above `Host` |

The preview runs `ssh -G -- <host>` and shows `user`, `hostname`, `port`, `remotecommand`,
`proxyjump`, so `Match`, `ProxyJump` and `%` tokens show what ssh will actually do. A broken
config shows ssh's own error there. Each host is resolved once per session.

## Config conventions

sshpick reads `~/.ssh/config` and follows its `Include` lines in the order ssh does. Two
comment conventions fill the columns. Nothing else in the config needs to change.

```text
# STAGING — shared test environment (rotates quarterly)
# jump host for the staging network
Host stg-bastion
  HostName 192.0.2.10

# consul server 1
Host stg-consul1 consul1.stg
  HostName 192.0.2.11
  ProxyJump stg-bastion
```

- Line 1 of an included file is the ENV label. Text from the first ` — `, ` - ` or ` (` on is
  dropped, so this file's label is `STAGING`.
- A comment directly above a `Host` line is its WHAT. A blank line or any other line in
  between clears it. Here `stg-bastion` gets `jump host for the staging network`. A `Host`
  on line 2 would get the label line as its WHAT too.
- `stg-consul1` is the row. `consul1.stg` is searchable, so typing it finds the row.
- `Host` follows ssh syntax: any case, tabs, `Host=x`, indentation. A name listed twice
  makes one row.
