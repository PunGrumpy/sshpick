-- keyway in Rex: keyway picks the host, Rex places it.
--
-- Copy or link this file next to init.lua, then:
--
--   require("keyway")
--   rex.bind("ctrl+a>s", "keyway")
--   rex.bind("ctrl+a>b", "keyway", { host = "stg-bastion" })  -- a favourite
--
-- One action, `keyway`:
--   no host   the hosts in Rex's command palette, from `keyway ls --recent --json`, the ones you
--             connected to last first. Typing filters in Rex; the "use typed text" row asks
--             keyway again, which also finds a host by alias, address or user. Enter on a row
--             opens that host as below; Esc ends the action. A Rex app too old for prompts gets
--             keyway's own picker in a floating layer instead.
--   host=...  a new tab labelled "host · ENV" running `keyway -- <host>`, which becomes ssh.
--
-- Nothing here reads ~/.ssh/config or ranks hosts. keyway is asked each time, so a host added a
-- minute ago is there without `rex config reload`. Loading this file does no work.
-- See docs/rex.md in the keyway repository.

local M = {}

-- Set `require("keyway").paths.keyway` (or `.rex`) in init.lua to skip the search below. Read
-- when the action runs, so installing keyway later needs no reload.
M.paths = { keyway = nil, rex = nil }

local TERMINAL = "com.superlogical.terminal.shell"
-- Rex refuses a window label over 128 characters.
local LABEL_LIMIT = 128
-- Rex refuses a prompt with more than this many items.
local PICK_LIMIT = 2000
-- The longest query shown in the prompt's title.
local QUERY_LIMIT = 60

-- The fallback layer's program. `$1` is keyway and `$2` is rex. keyway draws its picker on
-- /dev/tty under --print, so $(...) holds only the host. `rex do` types host= against this
-- action's schema, so the name stays a string. Esc (130) and Enter on no match (1) close the
-- layer quietly; any other failure stays on screen until Enter.
local HANDOFF = [[
hold() { printf '\nPress Enter to close.'; read -r _; exit "$1"; }
h=$("$1" --print)
s=$?
case $s in
  0) ;;
  1|130) exit 0 ;;
  *) hold "$s" ;;
esac
"$2" do keyway host="$h" >/dev/null || hold "$?"
]]

-- The tab's program. `$1` is keyway and `$2` the host. Rex keeps a tab open after any exit
-- inside its quick-exit window, a clean one too, so the tab decides: ssh's own failure (255)
-- waits for Enter, and a logout closes the tab. No --non-interactive: keyway remembers only a
-- connection made at a terminal, and the palette lists remembered hosts first. `$2` is a name
-- keyway just resolved, so it does not open its picker here.
local SESSION = [[
"$1" -- "$2"
s=$?
if [ "$s" -eq 255 ]; then
  printf '\nssh exited 255. Press Enter to close.'
  read -r _
fi
exit "$s"
]]

local function candidates(name)
  local home = os.getenv("HOME") or ""
  local list = {
    home .. "/.local/bin/" .. name,
    home .. "/.cargo/bin/" .. name,
    "/opt/homebrew/bin/" .. name,
    "/usr/local/bin/" .. name,
  }
  if name == "rex" then
    list[#list + 1] = "/Applications/Rex Beta.app/Contents/Helpers/Rex Server.rex/Contents/MacOS/rex"
    list[#list + 1] = home
      .. "/Library/Application Support/com.superlogical.rex/RexServer/Versions/Current/Rex Server.app/Contents/MacOS/rex"
  end
  return list
end

local function exists(path)
  local file = io.open(path, "r")
  if file == nil then
    return false
  end
  file:close()
  return true
end

-- The path of `name`, or an error naming where it looked. Rex's server runs actions with
-- PATH=/usr/bin:/bin:/usr/sbin:/sbin, so a bare `keyway` would not be found.
local function need(name)
  local chosen = M.paths[name]
  if chosen ~= nil then
    if exists(chosen) then
      return chosen
    end
    error(("%s not found at %s, which require(\"keyway\").paths.%s names.\nFix: point it at the %s binary, or remove the setting to search the usual places.")
      :format(name, chosen, name, name), 0)
  end
  local looked = candidates(name)
  for _, path in ipairs(looked) do
    if exists(path) then
      return path
    end
  end
  local fix = name == "keyway"
      and "run `make install` in the keyway repository"
    or "install Rex, or start it once so its CLI is in place"
  error(("%s not found in %s.\nFix: %s, or set require(\"keyway\").paths.%s in init.lua.")
    :format(name, table.concat(looked, ", "), fix, name), 0)
end

-- keyway's error, from the one JSON line it writes on stderr under --json, as
-- "message\nFix: fix". A keyway too old for --print answers in text; that text is the error.
local function failure(result)
  if result.timed_out then
    return "keyway gave no answer within 10 s"
  end
  local document = rex.json.decode(result.stderr)
  if type(document) ~= "table" or type(document.message) ~= "string" then
    local text = result.stderr:gsub("%s+$", "")
    if text == "" then
      return ("keyway exited %d"):format(result.code)
    end
    return text
  end
  if type(document.fix) == "string" then
    return document.message .. "\nFix: " .. document.fix
  end
  return document.message
end

-- `text` within `limit` bytes, which keeps it within `limit` characters however they are
-- counted. Cut at a character boundary, ending in "…" when cut.
local function cut(text, limit)
  if #text <= limit then
    return text
  end
  local keep = limit - #"…"
  local function continues(at)
    local byte = text:byte(at)
    return byte ~= nil and byte >= 0x80 and byte < 0xC0
  end
  while keep > 0 and continues(keep + 1) do
    keep = keep - 1
  end
  return text:sub(1, keep) .. "…"
end

-- "host · ENV", or the host alone when it has no ENV. The host comes first so a narrow tab
-- still shows it.
local function label(host)
  local text = host.host
  if host.env ~= "" then
    text = host.host .. " · " .. host.env
  end
  return cut(text, LABEL_LIMIT)
end

-- For a Rex app that cannot show prompts. The rex CLI is needed only here, to hand the host back.
local function open_tui_layer()
  local argv = { "/bin/sh", "-c", HANDOFF, "keyway-rex", need("keyway"), need("rex") }
  local layer, err = rex.session.new_layer{
    bounds = { 0.1, 0.1, 0.8, 0.8 },
    focus = true,
    layout = { block = {
      flavor = TERMINAL,
      label = "keyway",
      options = { shell = "none", cwd = os.getenv("HOME"), command = argv },
    } },
  }
  if layer == nil then
    error(err, 0)
  end
end

-- keyway decides what `name` means, from the config as it is now: an alias becomes its host, and
-- a host removed since the picker opened fails here, as an action error, not as a dead tab.
local function connect(name)
  local keyway = need("keyway")
  local result, err = rex.exec(
    { keyway, "--print", "--json", "--non-interactive", "--", name },
    { cwd = os.getenv("HOME"), timeout = 10 }
  )
  if result == nil then
    error(err, 0)
  end
  if not result.ok then
    error(failure(result), 0)
  end
  local host = rex.json.decode(result.stdout)
  if type(host) ~= "table" or type(host.host) ~= "string" then
    error("keyway printed no host: " .. result.stdout, 0)
  end
  local window, werr = rex.session.new_window{
    window_label = label(host),
    focus = true,
    layout = { block = {
      flavor = TERMINAL,
      label = host.host,
      options = {
        shell = "none",
        cwd = os.getenv("HOME"),
        command = { "/bin/sh", "-c", SESSION, "keyway-rex", keyway, host.host },
      },
    } },
  }
  if window == nil then
    error(werr, 0)
  end
  return { window_id = window.window_id, host = host.host }
end

-- 2345 as "2,345".
local function count(number)
  local text = tostring(number)
  local found
  repeat
    text, found = text:gsub("^(-?%d+)(%d%d%d)", "%1,%2")
  until found == 0
  return text
end

local function searching(query)
  return query ~= nil and query ~= ""
end

-- The prompt's title: what is listed, and of how many, for the query.
local function title(query, shown, matched)
  local cut_off = shown < matched
  if not searching(query) then
    if cut_off then
      return ("First %s of %s hosts"):format(count(shown), count(matched))
    end
    return "Connect to SSH Host"
  end
  local quoted = ('"%s"'):format(cut(query, QUERY_LIMIT))
  if cut_off then
    return ("First %s of %s hosts match %s"):format(count(shown), count(matched), quoted)
  end
  if matched == 0 then
    return "No host matches " .. quoted
  end
  if matched == 1 then
    return "1 host matches " .. quoted
  end
  return ("%s hosts match %s"):format(count(matched), quoted)
end

-- The hosts `keyway ls --recent --json` finds for `query`, or all of them, in the picker's
-- order. No match is exit 1 with an empty list; that is an answer, not a failure.
local function list(query)
  local argv = { need("keyway"), "ls", "--recent", "--json" }
  if searching(query) then
    -- After `--`, so a query that starts with `-` is text.
    argv[#argv + 1] = "--"
    argv[#argv + 1] = query
  end
  local result, err = rex.exec(argv, { cwd = os.getenv("HOME"), timeout = 10 })
  if result == nil then
    error(err, 0)
  end
  local document = rex.json.decode(result.stdout)
  if type(document) ~= "table" or type(document.hosts) ~= "table" then
    if result.ok then
      error("keyway printed no host list: " .. result.stdout, 0)
    end
    error(failure(result), 0)
  end
  for _, warning in ipairs(document.warnings or {}) do
    rex.log("warn", warning.message)
  end
  return document.hosts
end

-- One row per host, in keyway's order, up to what Rex takes. The id is the host, which `connect`
-- takes as it is.
local function rows(hosts)
  local items = {}
  for i = 1, math.min(#hosts, PICK_LIMIT) do
    local host = hosts[i]
    items[i] = { id = host.host, title = host.host, icon = "server" }
    if host.what ~= "" then
      items[i].subtitle = host.what
    end
    if host.env ~= "" then
      items[i].detail = host.env
    end
  end
  return items
end

local function refuse(message)
  return function()
    error(message, 0)
  end
end

-- What the action does when the prompt gives no answer, by Rex's reason. A cancel and a timeout
-- end it quietly.
local UNANSWERED = {
  cancelled = function() end,
  timeout = function() end,
  unsupported = open_tui_layer,
  no_client = refuse("no Rex app is connected to show the host list\nFix: open the Rex app, then run the action again."),
  busy = refuse("Rex is showing another prompt\nFix: answer or close it, then run the action again."),
  failed = function(detail)
    error("Rex could not show the host list" .. (detail and (": " .. detail) or ""), 0)
  end,
}

local function unanswered(reason, detail)
  local outcome = UNANSWERED[reason]
  if outcome == nil then
    error("Rex gave no answer to the host list: " .. tostring(reason), 0)
  end
  return outcome(detail)
end

-- Rex's own filter sees only the text of the rows. Typed text comes back from the "use typed
-- text" row, and keyway matches it again, by alias, address and user too. It is never a host to
-- connect to: a loose match must not open a host unasked, so the person always picks a row.
local function pick_in_palette()
  local query
  while true do
    local hosts = list(query)
    local items = rows(hosts)
    local answer, reason, detail = rex.ui.pick{
      title = title(query, #items, #hosts),
      placeholder = "Search host, ENV or what",
      items = items,
      allow_other = true,
    }
    if type(answer) == "table" then
      query = answer.text:match("^%s*(.-)%s*$")
    elseif answer ~= nil then
      return connect(answer)
    else
      return unanswered(reason, detail)
    end
  end
end

rex.action{
  name = "keyway",
  title = "Connect to SSH Host…",
  category = "SSH",
  keywords = "ssh keyway host server remote connect",
  description = "Pick a host from ~/.ssh/config in the command palette and open it in a new tab. host= skips the picker.",
  args = { host = "string?" },
  run = function(ctx, args)
    if args.host ~= nil then
      return connect(args.host)
    end
    return pick_in_palette()
  end,
}

return M
