# Rex

[Rex](https://www.superlogical.com/rex) places what sshpick picks. A key opens the hosts in Rex's
own command palette. Enter on a host opens a new tab labelled `host · ENV`, such as
`stg-consul1 · STAGING`, that runs ssh. Esc closes the palette and nothing else happens.

Each row shows the host, its WHAT under it and its ENV at the far end. The hosts you connected to
last come first, the way sshpick's own picker lists them, and the rest follow in the order of the
config. Typing filters the rows in Rex. A host that Rex's filter does not show, because you typed an alias, an address or a user,
is found by the "use typed text" row: it asks sshpick, which matches those too, and the palette
opens again with the hosts it found and a title that counts them. Nothing connects from typed
text. You always pick a row, so a loose match never opens a host unasked.

sshpick still reads `~/.ssh/config`, matches the hosts and resolves the exact name; Rex supplies
the palette, the tab, its label and the key. Nothing is cached on the Rex side: each key press
runs sshpick, so a host added a minute ago is listed without `rex config reload`.

The integration is one Lua file, [`integrations/rex/sshpick.lua`](../integrations/rex/sshpick.lua),
that defines one action, `sshpick`.

## Install

1. Install an sshpick that has `--print` and `ls --recent` (0.1.0 or newer): `make install` puts
   it in `~/.local/bin`. The palette needs both.
2. Copy or link the file next to your `init.lua`:

       ln -s "$PWD/integrations/rex/sshpick.lua" ~/.config/rex/sshpick.lua

   A copy works too; it does not follow later changes in the repository.
3. Add to `~/.config/rex/init.lua`:

   ```lua
   require("sshpick")
   rex.bind("ctrl+a>s", "sshpick")
   ```

4. Reload: `rex config reload`, or your reload key.

The action is also in the command palette as **Connect to SSH Host…**, under SSH.

Install the file on each machine whose Rex server you use. Actions run on the server, so a remote
Rex host uses its own sshpick, its own `~/.ssh/config`, and runs ssh from there.

## A favourite on a key

`host` skips the palette. The name may be a host or one of its aliases; sshpick turns it into the
host, so the tab gets the same label as a pick.

```lua
rex.bind("ctrl+a>b", "sshpick", { host = "stg-bastion" })
```

From a shell or a script:

```sh
rex do sshpick host=stg-bastion
```

## Where the binaries are

Rex's server runs actions with `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, so the action looks for
`sshpick` in `~/.local/bin`, `~/.cargo/bin`, `/opt/homebrew/bin` and `/usr/local/bin`. It looks
when the action runs, so installing sshpick later needs no reload. To name it yourself, in
`init.lua`:

```lua
local sshpick = require("sshpick")
sshpick.paths.sshpick = "/opt/tools/bin/sshpick"
```

The `rex` CLI is needed only by the fallback below. The action looks for it in the same places and
inside the Rex app, and `paths.rex` names it:

```lua
sshpick.paths.rex = "/Applications/Rex Beta.app/Contents/Helpers/Rex Server.rex/Contents/MacOS/rex"
```

## Many hosts

Rex shows at most 2,000 rows in one prompt. With more hosts than that, the palette lists the first
2,000, in the order sshpick gives them (recent hosts first), and its title says so, as `First 2,000 of 2,345 hosts`.
Type to narrow it: the "use typed text" row asks sshpick across all of them, and a query that
matches 2,000 or fewer shows every match.

## When the Rex app is too old

A Rex app that is too old to show prompts answers `unsupported`. The action then opens sshpick's
own picker in a floating layer instead. Enter closes the layer and opens the tab; Esc closes the
layer and nothing else happens. This path needs the `rex` CLI, which hands the picked host back to
the action, and an sshpick that has `--print`. Updating the Rex app turns the palette on with no
change here.

## How it works

Without `host`, the action runs `sshpick ls --recent --json`, with the query after `--` once you
have typed one, so a query that starts with `-` is text. `--recent` gives the picker's order and
reads the history only. The tab connects without `--non-interactive`, so sshpick remembers the
host as it does at any terminal, and it is first in the palette the next time. Each host in the
answer becomes a row, in that order: the host is its title and its id, the WHAT its second line
and the ENV its far end. Either is left out when empty. The action asks Rex to show them with
`rex.ui.pick`. What comes back is one of three things:

- A row's id. The action takes the path below with it.
- Typed text. The action runs `sshpick ls --recent --json -- <text>` and asks again.
- No answer, with a reason. `cancelled` and `timeout` end the action quietly. `unsupported` opens
  the layer above. `no_client`, `busy`, `failed` and any reason Rex adds later are an action error.

A query that matches no host is not an error: the palette opens with no rows and a title that says
`No host matches "..."`. Warnings in the config, such as a bad `Include`, go to Rex's log at level
`warn`.

With `host`, the action runs `sshpick --print --json --non-interactive -- <host>`. sshpick checks
the name against the config as it is now and returns `{host, env, what, names}`. The action then
opens a focused tab, labelled `host · ENV` (the host alone when it has no ENV, cut to Rex's 128
characters), whose terminal runs `sshpick -- <host>`. That prints
`> ssh -- host` and becomes ssh, so sshpick stays the only thing that runs ssh. A short `/bin/sh`
script around it holds the tab when ssh exits 255, its own failure, because Rex keeps a tab open
after any exit in its quick-exit window and closes it after any exit later.

The fallback layer's terminal runs a short `/bin/sh` script: it runs `sshpick --print`, which
draws the picker on the layer's terminal and prints the picked host, then runs
`rex do sshpick host=<host>`. Rex reads `host=` against the action's schema, so a name such as
`007` stays a string.

## When something fails

| Case | What you see |
|------|--------------|
| sshpick is in none of the places above | An action error that lists where it looked; fix with `make install` or `paths.sshpick` |
| The config cannot be read, or has no hosts | An action error with sshpick's message and `Fix:`; no palette opens |
| You press Esc, or the palette times out | The palette closes; nothing else happens |
| No Rex app is connected | An action error: open the Rex app and run the action again |
| Another prompt is open in Rex | An action error: answer or close it, then run the action again |
| Rex fails to show the palette | An action error with the detail Rex gave |
| The Rex app is too old for prompts | sshpick's own picker opens in a layer, as above |
| That layer needs the `rex` CLI and it is not found | An action error for `paths.rex`, before any layer opens |
| sshpick is older than `ls --recent` | An action error with sshpick's `unknown option "--recent"` and its `Fix:`; no palette opens. Install a newer sshpick |
| sshpick is older than `--print` | In the layer, `Error: unknown option "--print"`, and it waits for Enter |
| `host=` names no host, or matches one only loosely | An action error with sshpick's message and `Fix:`; no tab opens |
| ssh cannot connect (DNS, refused, auth): it exits 255 | The tab shows ssh's message and closes on Enter |
| You log out | The tab closes |

## Check it live

1. `rex config check ~/.config/rex/init.lua` reports `actions: 1` and no complaints.
2. After the reload, `rex actions` lists `sshpick`.
3. `rex do sshpick host=<a host>` opens a tab labelled with its host and ENV.
4. `ctrl+a s`: the palette lists your hosts, the ones you used last first, each with its WHAT and
   ENV. Pick one with Enter: a
   labelled tab opens. Press `ctrl+a s` again, then Esc: no tab opens.
5. `ctrl+a s`, then type a host's alias or address and choose the "use typed text" row: the palette
   opens again with the hosts sshpick found, and its title counts them.
6. Add a `Host` to a file in `config.d`, then `ctrl+a s`: it is listed, with no reload.
7. Set `paths.sshpick = "/nope"`, reload, then `ctrl+a s`: the action error names the fix.
8. With an older Rex app that cannot show prompts, `ctrl+a s` opens sshpick's picker in a layer.
