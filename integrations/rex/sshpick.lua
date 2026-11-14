-- sshpick in Rex: sshpick picks the host, Rex places it.
--
-- Copy or link this file next to init.lua, then:
--
--   require("sshpick")
--   rex.bind("ctrl+a>s", "sshpick")
--   rex.bind("ctrl+a>b", "sshpick", { host = "stg-bastion" })  -- a favourite
--
-- One action, `sshpick`:
--   no host   sshpick's own picker in a floating layer. Enter runs this action again with the
--             picked host; Esc closes the layer.
--   host=...  a new tab labelled "host · ENV" running `sshpick --non-interactive -- <host>`,
--             which becomes ssh.
--
-- Nothing here reads ~/.ssh/config or ranks hosts. sshpick is asked each time, so a host added a
-- minute ago is there without `rex config reload`. Loading this file does no work.
-- See docs/rex.md in the sshpick repository.

local M = {}

-- Set `require("sshpick").paths.sshpick` (or `.rex`) in init.lua to skip the search below. Read
-- when the action runs, so installing sshpick later needs no reload.
M.paths = { sshpick = nil, rex = nil }

local TERMINAL = "com.superlogical.terminal.shell"
-- Rex refuses a window label over 128 characters.
local LABEL_LIMIT = 128

-- The picker layer's program. `$1` is sshpick and `$2` is rex. sshpick draws its picker on
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
"$2" do sshpick host="$h" >/dev/null || hold "$?"
]]

-- The tab's program. `$1` is sshpick and `$2` the host. Rex keeps a tab open after any exit
-- inside its quick-exit window, a clean one too, so the tab decides: ssh's own failure (255)
-- waits for Enter, and a logout closes the tab.
local SESSION = [[
"$1" --non-interactive -- "$2"
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
-- PATH=/usr/bin:/bin:/usr/sbin:/sbin, so a bare `sshpick` would not be found.
local function need(name)
  local chosen = M.paths[name]
  if chosen ~= nil then
    if exists(chosen) then
      return chosen
    end
    error(("%s not found at %s, which require(\"sshpick\").paths.%s names.\nFix: point it at the %s binary, or remove the setting to search the usual places.")
      :format(name, chosen, name, name), 0)
  end
  local looked = candidates(name)
  for _, path in ipairs(looked) do
    if exists(path) then
      return path
    end
  end
  local fix = name == "sshpick"
      and "run `make install` in the sshpick repository"
    or "install Rex, or start it once so its CLI is in place"
  error(("%s not found in %s.\nFix: %s, or set require(\"sshpick\").paths.%s in init.lua.")
    :format(name, table.concat(looked, ", "), fix, name), 0)
end

-- sshpick's error, from the one JSON line it writes on stderr under --json, as
-- "message\nFix: fix". An sshpick too old for --print answers in text; that text is the error.
local function failure(result)
  if result.timed_out then
    return "sshpick gave no answer within 10 s"
  end
  local document = rex.json.decode(result.stderr)
  if type(document) ~= "table" or type(document.message) ~= "string" then
    local text = result.stderr:gsub("%s+$", "")
    if text == "" then
      return ("sshpick exited %d"):format(result.code)
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

local function open_picker()
  local argv = { "/bin/sh", "-c", HANDOFF, "sshpick-rex", need("sshpick"), need("rex") }
  local layer, err = rex.session.new_layer{
    bounds = { 0.1, 0.1, 0.8, 0.8 },
    focus = true,
    layout = { block = {
      flavor = TERMINAL,
      label = "sshpick",
      options = { shell = "none", cwd = os.getenv("HOME"), command = argv },
    } },
  }
  if layer == nil then
    error(err, 0)
  end
end

-- sshpick decides what `name` means, from the config as it is now: an alias becomes its host, and
-- a host removed since the picker opened fails here, as an action error, not as a dead tab.
local function connect(name)
  local sshpick = need("sshpick")
  local result, err = rex.exec(
    { sshpick, "--print", "--json", "--non-interactive", "--", name },
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
    error("sshpick printed no host: " .. result.stdout, 0)
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
        command = { "/bin/sh", "-c", SESSION, "sshpick-rex", sshpick, host.host },
      },
    } },
  }
  if window == nil then
    error(werr, 0)
  end
  return { window_id = window.window_id, host = host.host }
end

rex.action{
  name = "sshpick",
  title = "Connect to SSH Host…",
  category = "SSH",
  keywords = "ssh sshpick host server remote connect",
  description = "Pick a host from ~/.ssh/config and open it in a new tab. host= skips the picker.",
  args = { host = "string?" },
  run = function(ctx, args)
    if args.host ~= nil then
      return connect(args.host)
    end
    open_picker()
  end,
}

return M
