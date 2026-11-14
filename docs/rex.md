# Rex

[Rex](https://www.superlogical.com/rex) places what sshpick picks. A key opens sshpick's own
picker in a floating layer over the current tab. Enter closes the layer and opens a new tab
labelled `host · ENV`, such as `stg-consul1 · STAGING`, that runs ssh. Esc closes the layer and
nothing else happens.

sshpick still reads `~/.ssh/config`, ranks the hosts and resolves the exact name; Rex supplies the
layer, the tab, its label and the key. Nothing is cached on the Rex side: each key press runs
sshpick, so a host added a minute ago is listed without `rex config reload`.

The integration is one Lua file, [`integrations/rex/sshpick.lua`](../integrations/rex/sshpick.lua),
that defines one action, `sshpick`.

## Install

1. Install an sshpick that has `--print` (newer than 0.2.0): `make install` puts it in
   `~/.local/bin`.
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

`host` skips the picker. The name may be a host or one of its aliases; sshpick turns it into the
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
`sshpick` in `~/.local/bin`, `~/.cargo/bin`, `/opt/homebrew/bin` and `/usr/local/bin`, and for the
`rex` CLI in the same places and inside the Rex app. It looks when the action runs, so installing
sshpick later needs no reload. To name them yourself, in `init.lua`:

```lua
local sshpick = require("sshpick")
sshpick.paths.sshpick = "/opt/tools/bin/sshpick"
sshpick.paths.rex = "/Applications/Rex Beta.app/Contents/Helpers/Rex Server.rex/Contents/MacOS/rex"
```

## How it works

Without `host`, the action opens a layer whose terminal runs a short `/bin/sh` script: it runs
`sshpick --print`, which draws the picker on the layer's terminal and prints the picked host,
then runs `rex do sshpick host=<host>`. That second run takes the path below. Rex reads `host=`
against the action's schema, so a name such as `007` stays a string.

With `host`, the action runs `sshpick --print --json --non-interactive -- <host>`. sshpick checks
the name against the config as it is now and returns `{host, env, what, names}`. The action then
opens a focused tab, labelled `host · ENV` (the host alone when it has no ENV, cut to Rex's 128
characters), whose terminal runs `sshpick --non-interactive -- <host>`. That prints
`> ssh -- host` and becomes ssh, so sshpick stays the only thing that runs ssh. A short `/bin/sh`
script around it holds the tab when ssh exits 255, its own failure, because Rex keeps a tab open
after any exit in its quick-exit window and closes it after any exit later.

## When something fails

| Case | What you see |
|------|--------------|
| sshpick is in none of the places above | An action error that lists where it looked; fix with `make install` or `paths.sshpick` |
| The `rex` CLI is not found | The same, for `paths.rex`, before any layer opens |
| sshpick is older than `--print` | The layer shows `Error: unknown option "--print"` and waits for Enter |
| The config cannot be read, or has no hosts | The layer shows sshpick's error and its `Fix:`, and waits for Enter |
| Esc in the picker, or Enter with no match | The layer closes; nothing else happens |
| `host=` names no host, or matches one only loosely | An action error with sshpick's message and `Fix:`; no tab opens |
| ssh cannot connect (DNS, refused, auth): it exits 255 | The tab shows ssh's message and closes on Enter |
| You log out | The tab closes |

## Check it live

1. `sshpick --print` in a Rex tab: the picker opens; Enter prints the host.
2. `rex config check ~/.config/rex/init.lua` reports `actions: 1` and no complaints.
3. After the reload, `rex actions` lists `sshpick`.
4. `rex do sshpick host=<a host>` opens a tab labelled with its host and ENV.
5. `ctrl+a s`, type part of a name, Enter: the layer closes and a labelled tab opens. Press
   `ctrl+a s` again, then Esc: the layer closes and no tab opens.
6. Add a `Host` to a file in `config.d`, then `ctrl+a s`: it is listed, with no reload.
7. Set `paths.sshpick = "/nope"`, reload, then `ctrl+a s`: the action error names the fix.
