//! The shipped Rex integration, `integrations/rex/sshpick.lua`, loaded in Lua 5.1 as Rex runs it,
//! against a stub `rex` table. `rex.exec` runs the real programs with `HOME` at a fixture home and
//! the fake `ssh` first on `PATH`, so these tests catch a flag sshpick no longer takes.
//! `rex.ui.pick` answers from a script each test sets: a row, typed text, or no answer and why.
//! The fallback layer's hand-off is taken from the recorded `new_layer` call and run as Rex would
//! run it: through a pipe, and on a pty with a person typing.

#[path = "common/pty.rs"]
#[allow(dead_code)]
mod pty;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Stdio};

use mlua::{Lua, Table, Value};
use pty::{Exit, Options, Session};
use serde_json::json;
use test_case::test_case;

const SSHPICK: &str = env!("CARGO_BIN_EXE_sshpick");
const TERMINAL: &str = "com.superlogical.terminal.shell";

/// The parts of `rex` the integration uses. Every call is recorded in `rex._calls`.
const STUB: &str = r#"
local calls = {}
local real_getenv = os.getenv
os.getenv = function(name)
  if name == "HOME" then return __home end
  return real_getenv(name)
end
rex = {
  _calls = calls,
  _actions = {},
  _answers = {},
  action = function(spec) table.insert(rex._actions, spec) end,
  session = {
    new_window = function(args)
      table.insert(calls, { call = "new_window", args = args })
      return { window_id = "window:1", block_ids = { "block:1" }, revision = 1 }
    end,
    new_layer = function(args)
      table.insert(calls, { call = "new_layer", args = args })
      return { block_ids = { "block:2" }, revision = 2 }
    end,
  },
  ui = {
    pick = function(spec)
      table.insert(calls, { call = "pick", args = spec })
      local answer = table.remove(rex._answers, 1)
      if answer == nil then
        error("rex.ui.pick was called with no answer scripted", 0)
      end
      return unpack(answer, 1, answer.n)
    end,
  },
  log = function(level, message)
    table.insert(calls, { call = "log", level = level, message = message })
  end,
  json = { decode = __decode },
  exec = function(argv, opts)
    table.insert(calls, { call = "exec", argv = argv, opts = opts })
    return __exec(argv)
  end,
}
"#;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn integration_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../integrations/rex")
}

fn fake(name: &str) -> String {
    fixtures().join("rex").join(name).display().to_string()
}

fn search_path() -> String {
    format!(
        "{}:{}",
        fixtures().join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// The program a host's tab runs: the shipped `SESSION` script around sshpick.
fn session_argv(host: &str) -> serde_json::Value {
    let lua = std::fs::read_to_string(integration_dir().join("sshpick.lua")).or_panic();
    let start = lua.find("local SESSION = [[\n").or_panic() + "local SESSION = [[\n".len();
    let end = start + lua[start..].find("]]").or_panic();
    json!([
        "/bin/sh",
        "-c",
        &lua[start..end],
        "sshpick-rex",
        SSHPICK,
        host
    ])
}

/// A scratch file under cargo's test directory, gone before the test starts.
fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("rex-{name}-{}", process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

/// `unwrap`, for the helpers here, which are not `#[test]` functions.
trait OrPanic<T> {
    fn or_panic(self) -> T;
}

impl<T, E: std::fmt::Display> OrPanic<T> for Result<T, E> {
    #[track_caller]
    fn or_panic(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{error}"),
        }
    }
}

impl<T> OrPanic<T> for Option<T> {
    #[track_caller]
    fn or_panic(self) -> T {
        match self {
            Some(value) => value,
            None => panic!("nothing there"),
        }
    }
}

/// What `rex.ui.pick` gives back, in the order Rex's docs name them.
enum Answer<'a> {
    /// The id of the row the person chose.
    Id(&'a str),
    /// The text typed on the "use typed text" row.
    Text(&'a str),
    /// No answer: Rex's reason, and its detail for `failed`.
    Unanswered(&'a str, Option<&'a str>),
}

struct Rex {
    lua: Lua,
}

impl Rex {
    /// The shipped file required by name, as `init.lua` does, with `HOME` at the fixture `home`
    /// and both binaries set in `paths`: the built sshpick and the fake rex.
    fn load(home: &str) -> Rex {
        let rex = Rex::bare(home);
        rex.set_paths(Some(SSHPICK), Some(&fake("rex")));
        rex
    }

    /// `home` is a fixture's name, or an absolute path, which `join` takes as it is.
    fn bare(home: &str) -> Rex {
        let lua = Lua::new();
        let home = fixtures().join(home);
        let globals = lua.globals();
        let decode = lua
            .create_function(|lua, text: String| {
                Ok(match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(value) => (to_lua(lua, &value)?, Value::Nil),
                    Err(error) => (
                        Value::Nil,
                        Value::String(lua.create_string(error.to_string())?),
                    ),
                })
            })
            .or_panic();
        let exec_home = home.clone();
        let exec = lua
            .create_function(move |lua, argv: Vec<String>| {
                let Some((program, args)) = argv.split_first() else {
                    return Ok((Value::Nil, Value::String(lua.create_string("empty argv")?)));
                };
                let output = process::Command::new(program)
                    .args(args)
                    .env("HOME", &exec_home)
                    .env("PATH", search_path())
                    .env_remove("TERM")
                    .stdin(Stdio::null())
                    .output();
                let output = match output {
                    Ok(output) => output,
                    Err(error) => {
                        return Ok((
                            Value::Nil,
                            Value::String(lua.create_string(error.to_string())?),
                        ));
                    }
                };
                let result = lua.create_table()?;
                let code = output.status.code().unwrap_or(-1);
                result.set("code", code)?;
                result.set("ok", code == 0)?;
                result.set("timed_out", false)?;
                result.set("stdout", lua.create_string(&output.stdout)?)?;
                result.set("stderr", lua.create_string(&output.stderr)?)?;
                Ok((Value::Table(result), Value::Nil))
            })
            .or_panic();
        globals.set("__home", home.display().to_string()).or_panic();
        globals.set("__decode", decode).or_panic();
        globals.set("__exec", exec).or_panic();
        lua.load(STUB).exec().or_panic();
        let package: Table = globals.get("package").or_panic();
        package
            .set("path", format!("{}/?.lua", integration_dir().display()))
            .or_panic();
        lua.load(r#"require("sshpick")"#).exec().or_panic();
        Rex { lua }
    }

    fn set_paths(&self, sshpick: Option<&str>, rex: Option<&str>) {
        let paths: Table = self
            .lua
            .load(r#"return require("sshpick").paths"#)
            .eval()
            .or_panic();
        paths.set("sshpick", sshpick).or_panic();
        paths.set("rex", rex).or_panic();
    }

    fn actions(&self) -> Vec<Table> {
        let rex: Table = self.lua.globals().get("rex").or_panic();
        let actions: Table = rex.get("_actions").or_panic();
        actions.sequence_values().map(OrPanic::or_panic).collect()
    }

    /// Runs the action as a key, the palette or `rex do` would, and returns its result or the
    /// message it failed with.
    fn run(&self, args: serde_json::Value) -> Result<serde_json::Value, String> {
        let [action] = &self.actions()[..] else {
            panic!("one action");
        };
        let run: mlua::Function = action.get("run").or_panic();
        let ctx = self.lua.create_table().or_panic();
        let args = to_lua(&self.lua, &args).or_panic();
        match run.call::<Value>((ctx, args)) {
            Ok(value) => Ok(to_json(&value)),
            Err(error) => Err(message(&error)),
        }
    }

    /// Queues what the next `rex.ui.pick` calls return; a call with nothing queued fails.
    fn script(&self, answers: &[Answer]) {
        let rex: Table = self.lua.globals().get("rex").or_panic();
        let queue: Table = rex.get("_answers").or_panic();
        for answer in answers {
            let entry = self.lua.create_table().or_panic();
            match answer {
                Answer::Id(id) => {
                    entry.set("n", 1).or_panic();
                    entry.set(1, *id).or_panic();
                }
                Answer::Text(text) => {
                    let typed = self.lua.create_table().or_panic();
                    typed.set("text", *text).or_panic();
                    entry.set("n", 1).or_panic();
                    entry.set(1, typed).or_panic();
                }
                Answer::Unanswered(reason, detail) => {
                    entry.set("n", 3).or_panic();
                    entry.set(2, *reason).or_panic();
                    entry.set(3, *detail).or_panic();
                }
            }
            queue.push(entry).or_panic();
        }
    }

    fn calls(&self) -> serde_json::Value {
        let rex: Table = self.lua.globals().get("rex").or_panic();
        to_json(&rex.get::<Value>("_calls").or_panic())
    }

    fn calls_of(&self, kind: &str) -> Vec<serde_json::Value> {
        self.calls()
            .as_array()
            .into_iter()
            .flatten()
            .filter(|call| call["call"] == kind)
            .cloned()
            .collect()
    }

    fn call(&self, kind: &str) -> serde_json::Value {
        let found = self.calls_of(kind);
        let [call] = &found[..] else {
            panic!("one {kind} call in {:#}", self.calls());
        };
        call.clone()
    }

    /// Every call so far, by kind, in order.
    fn kinds(&self) -> Vec<String> {
        self.calls()
            .as_array()
            .into_iter()
            .flatten()
            .map(|call| call["call"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    /// The `title` and row ids of every prompt so far.
    fn prompts(&self) -> Vec<(String, Vec<String>)> {
        self.calls_of("pick")
            .iter()
            .map(|pick| {
                let ids = pick["args"]["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item["id"].as_str().map(str::to_owned))
                    .collect();
                (
                    pick["args"]["title"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    ids,
                )
            })
            .collect()
    }

    /// The argv the fallback layer's terminal runs, with `sshpick` as the program it runs. The
    /// layer opens only after the palette listed hosts with the built sshpick, so a fake one is
    /// put in afterwards.
    fn handoff(&self, sshpick: &str) -> Vec<String> {
        self.script(&[Answer::Unanswered("unsupported", None)]);
        assert_eq!(self.run(json!({})), Ok(json!(null)));
        let layer = self.call("new_layer");
        let argv = &layer["args"]["layout"]["block"]["options"]["command"];
        let mut argv: Vec<String> = serde_json::from_value(argv.clone()).or_panic();
        argv[4] = sshpick.to_owned();
        argv
    }
}

/// What `error(message, 0)` raised, without the traceback mlua adds.
fn message(error: &mlua::Error) -> String {
    match error {
        mlua::Error::RuntimeError(text) => text
            .split("\nstack traceback:")
            .next()
            .unwrap_or_default()
            .to_owned(),
        mlua::Error::CallbackError { cause, .. } => message(cause),
        other => other.to_string(),
    }
}

fn to_lua(lua: &Lua, value: &serde_json::Value) -> mlua::Result<Value> {
    Ok(match value {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(flag) => Value::Boolean(*flag),
        serde_json::Value::Number(number) => Value::Number(number.as_f64().unwrap_or_default()),
        serde_json::Value::String(text) => Value::String(lua.create_string(text)?),
        serde_json::Value::Array(items) => {
            let table = lua.create_table()?;
            for item in items {
                table.push(to_lua(lua, item)?)?;
            }
            Value::Table(table)
        }
        serde_json::Value::Object(fields) => {
            let table = lua.create_table()?;
            for (key, field) in fields {
                table.set(key.as_str(), to_lua(lua, field)?)?;
            }
            Value::Table(table)
        }
    })
}

/// A table with keys 1 to n is an array, any other an object, as `rex.json.encode` has it.
fn to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Nil => serde_json::Value::Null,
        Value::Boolean(flag) => json!(flag),
        Value::Integer(number) => json!(number),
        Value::Number(number) if number.fract() == 0.0 => json!(*number as i64),
        Value::Number(number) => json!(number),
        Value::String(text) => json!(text.to_str().or_panic().to_owned()),
        Value::Table(table) => {
            let length = table.raw_len();
            let pairs: Vec<(Value, Value)> = table.pairs().map(OrPanic::or_panic).collect();
            if length > 0 && pairs.len() == length {
                json!(
                    table
                        .sequence_values::<Value>()
                        .map(|item| to_json(&item.or_panic()))
                        .collect::<Vec<_>>()
                )
            } else {
                let mut object = serde_json::Map::new();
                for (key, field) in pairs {
                    let key = match key {
                        Value::String(text) => text.to_str().or_panic().to_owned(),
                        other => format!("{other:?}"),
                    };
                    object.insert(key, to_json(&field));
                }
                serde_json::Value::Object(object)
            }
        }
        Value::Function(_) => json!("<function>"),
        other => json!(format!("{other:?}")),
    }
}

#[test]
fn loading_defines_one_action_and_does_nothing_else() {
    let rex = Rex::bare("inspect");
    let [action] = &rex.actions()[..] else {
        panic!("one action");
    };
    let get = |key: &str| to_json(&action.get::<Value>(key).or_panic());
    assert_eq!(get("name"), "sshpick");
    assert_eq!(get("title"), "Connect to SSH Host…");
    assert_eq!(get("category"), "SSH");
    assert_eq!(get("args"), json!({ "host": "string?" }));
    assert_eq!(
        rex.calls(),
        json!({}),
        "no exec and no session call at load"
    );
}

#[test]
fn a_host_opens_a_focused_tab_labelled_with_its_env_that_runs_sshpick_under_a_hold_on_255() {
    let rex = Rex::load("inspect");
    let result = rex.run(json!({ "host": "consul1.stg" }));
    assert_eq!(
        result,
        Ok(json!({ "window_id": "window:1", "host": "stg-consul1" }))
    );
    let exec = rex.call("exec");
    assert_eq!(
        exec["argv"],
        json!([
            SSHPICK,
            "--print",
            "--json",
            "--non-interactive",
            "--",
            "consul1.stg"
        ])
    );
    assert_eq!(exec["opts"]["timeout"], 10);
    let home = fixtures().join("inspect").display().to_string();
    assert_eq!(
        rex.call("new_window")["args"],
        json!({
            "window_label": "stg-consul1 · STAGING",
            "focus": true,
            "layout": { "block": {
                "flavor": TERMINAL,
                "label": "stg-consul1",
                "options": {
                    "shell": "none",
                    "cwd": home,
                    "command": session_argv("stg-consul1"),
                },
            } },
        })
    );
}

#[test_case("names", "pg-primary", "db-prod"; "no env is the host alone")]
#[test_case("home", "only-host", "only-host"; "a host in the root config has no env")]
#[test_case("inspect", "stg-consul2", "stg-consul2 · STAGING"; "an env from the included file")]
fn the_tab_label_is_host_dot_env(home: &str, host: &str, want: &str) {
    let rex = Rex::load(home);
    rex.run(json!({ "host": host })).or_panic();
    assert_eq!(rex.call("new_window")["args"]["window_label"], want);
}

#[test]
fn a_long_label_is_cut_to_what_rex_takes() {
    let rex = Rex::load("rex-long");
    rex.run(json!({ "host": "long-host" })).or_panic();
    let label = rex.call("new_window")["args"]["window_label"]
        .as_str()
        .or_panic()
        .to_owned();
    assert!(label.len() <= 128, "{} bytes: {label}", label.len());
    assert!(label.starts_with("long-host · ÉNVÉNV"), "{label}");
    assert!(label.ends_with('…'), "{label}");
}

#[test_case("names", "nope", "no host matches \"nope\"\nFix: list every host with `sshpick ls`."; "no such host")]
#[test_case("names", "db-pr", "no host is named \"db-pr\"; \"db-prod\" matches loosely\nFix: connect with `sshpick --non-interactive db-prod`, or list candidates with `sshpick ls db-pr`."; "a loose match")]
#[test_case("names", "web-", "no host is named \"web-\"; \"web-2\" matches loosely\nFix: connect with `sshpick --non-interactive web-2`, or list candidates with `sshpick ls web-`."; "a prefix is loose too")]
#[test_case("empty", "x", "no hosts found in ~/.ssh/config or the files it Includes\nFix: add `Host` lines; ENV and WHAT come from comments."; "no hosts")]
fn a_host_sshpick_refuses_is_an_action_error_with_its_fix(home: &str, host: &str, want: &str) {
    let rex = Rex::load(home);
    assert_eq!(rex.run(json!({ "host": host })), Err(want.to_owned()));
    let calls = rex.calls();
    assert!(
        calls
            .as_array()
            .into_iter()
            .flatten()
            .all(|call| call["call"] == "exec"),
        "no tab opens: {calls:#}"
    );
}

#[test]
fn an_sshpick_too_old_for_print_is_named_by_its_own_text() {
    let rex = Rex::load("names");
    rex.set_paths(Some(&fake("sshpick-old")), None);
    assert_eq!(
        rex.run(json!({ "host": "web" })),
        Err("Error: unknown option \"--print\"\n\
             Fix: sshpick accepts --help, --no-color, --non-interactive, --version."
            .to_owned())
    );
}

#[test]
fn a_binary_set_in_paths_that_is_not_there_is_an_error_naming_it() {
    let rex = Rex::load("names");
    rex.set_paths(Some("/nonexistent/sshpick"), None);
    let error = rex.run(json!({ "host": "web" })).unwrap_err();
    assert!(
        error.starts_with("sshpick not found at /nonexistent/sshpick"),
        "{error}"
    );
    assert!(error.contains("\nFix: "), "{error}");
    assert_eq!(rex.calls(), json!({}), "nothing ran");
}

#[test_case("sshpick", json!({ "host": "web" }), &[]; "sshpick for a host")]
#[test_case("sshpick", json!({}), &[]; "sshpick for the palette")]
#[test_case("rex", json!({}), &["exec", "pick"]; "rex for the fallback layer, after the palette is tried")]
fn a_binary_in_none_of_the_usual_places_is_an_error_listing_them(
    name: &str,
    args: serde_json::Value,
    ran: &[&str],
) {
    let rex = Rex::load("names");
    rex.script(&[Answer::Unanswered("unsupported", None)]);
    let other = if name == "sshpick" {
        rex.set_paths(None, Some(&fake("rex")));
        fake("rex")
    } else {
        rex.set_paths(Some(SSHPICK), None);
        SSHPICK.to_owned()
    };
    // Only the other binary exists, wherever `name` is looked for.
    rex.lua.globals().set("__other", other).or_panic();
    rex.lua
        .load(
            "local real = io.open
             io.open = function(path, mode)
               if path == __other then return real(path, mode) end
               return nil
             end",
        )
        .exec()
        .or_panic();
    let Err(error) = rex.run(args) else {
        panic!("the action ran");
    };
    let home = fixtures().join("names").display().to_string();
    assert!(
        error.starts_with(&format!(
            "{name} not found in {home}/.local/bin/{name}, {home}/.cargo/bin/{name}, \
             /opt/homebrew/bin/{name}, /usr/local/bin/{name}"
        )),
        "{error}"
    );
    assert!(
        error.contains(&format!(
            "set require(\"sshpick\").paths.{name} in init.lua"
        )),
        "{error}"
    );
    assert_eq!(rex.kinds(), ran, "no tab and no layer opens");
}

const CANCELLED: Answer = Answer::Unanswered("cancelled", None);

#[test]
fn no_host_lists_the_hosts_in_the_palette_and_a_pick_opens_the_tab_host_would() {
    let rex = Rex::bare("ui");
    rex.set_paths(Some(SSHPICK), None);
    rex.script(&[Answer::Id("newstg-consul2")]);
    assert_eq!(
        rex.run(json!({})),
        Ok(json!({ "window_id": "window:1", "host": "newstg-consul2" }))
    );
    assert_eq!(rex.kinds(), ["exec", "pick", "exec", "new_window"]);

    let list = &rex.calls_of("exec")[0];
    assert_eq!(list["argv"], json!([SSHPICK, "ls", "--json"]));
    assert_eq!(list["opts"]["timeout"], 10);
    assert_eq!(
        list["opts"]["cwd"],
        fixtures().join("ui").display().to_string()
    );
    assert_eq!(
        rex.call("pick")["args"],
        json!({
            "title": "Connect to SSH Host",
            "placeholder": "Search host, ENV or what",
            "allow_other": true,
            "items": [
                { "id": "lab-a", "title": "lab-a", "icon": "server",
                  "subtitle": "LAB — personal VMs", "detail": "LAB" },
                { "id": "lab-b", "title": "lab-b", "icon": "server",
                  "subtitle": "build box", "detail": "LAB" },
                { "id": "newstg-consul1", "title": "newstg-consul1", "icon": "server",
                  "subtitle": "consul server 1 — mgmt", "detail": "NEW STAGING" },
                { "id": "newstg-consul2", "title": "newstg-consul2", "icon": "server",
                  "detail": "NEW STAGING" },
                { "id": "newstg-bastion", "title": "newstg-bastion", "icon": "server",
                  "subtitle": "bastion", "detail": "NEW STAGING" },
            ],
        })
    );

    let by_name = Rex::load("ui");
    by_name.run(json!({ "host": "newstg-consul2" })).or_panic();
    assert_eq!(rex.call("new_window"), by_name.call("new_window"));
}

#[test_case("web", "2 hosts match \"web\"", &["web", "web-2"]; "several hosts")]
#[test_case("pg-primary", "1 host matches \"pg-primary\"", &["db-prod"]; "an alias no row shows")]
#[test_case("192.0.2.3", "1 host matches \"192.0.2.3\"", &["db-prod"]; "an address no row shows")]
#[test_case("-2", "1 host matches \"-2\"", &["web-2"]; "a query that starts with a dash is text")]
fn typed_text_asks_sshpick_again_and_the_title_counts_what_it_found(
    typed: &str,
    title: &str,
    ids: &[&str],
) {
    let rex = Rex::load("names");
    rex.script(&[Answer::Text(typed), CANCELLED]);
    assert_eq!(rex.run(json!({})), Ok(json!(null)));
    assert_eq!(rex.kinds(), ["exec", "pick", "exec", "pick"]);
    assert_eq!(
        rex.calls_of("exec")[1]["argv"],
        json!([SSHPICK, "ls", "--json", "--", typed])
    );
    let prompts = rex.prompts();
    assert_eq!(prompts[0].0, "Connect to SSH Host");
    assert_eq!(
        prompts[1],
        (
            title.to_owned(),
            ids.iter().map(|id| (*id).to_owned()).collect()
        )
    );
}

#[test]
fn a_query_that_matches_nothing_asks_again_and_a_later_pick_still_connects() {
    let rex = Rex::load("names");
    rex.script(&[
        Answer::Text("zzz"),
        Answer::Text("  pg-primary "),
        Answer::Id("db-prod"),
    ]);
    assert_eq!(
        rex.run(json!({})),
        Ok(json!({ "window_id": "window:1", "host": "db-prod" }))
    );
    let titles: Vec<String> = rex.prompts().into_iter().map(|(title, _)| title).collect();
    assert_eq!(
        titles,
        [
            "Connect to SSH Host",
            "No host matches \"zzz\"",
            "1 host matches \"pg-primary\""
        ]
    );
    assert_eq!(rex.prompts()[1].1, Vec::<String>::new());
    assert_eq!(rex.call("new_window")["args"]["window_label"], "db-prod");
}

#[test_case("cancelled"; "esc")]
#[test_case("timeout"; "no answer in time")]
fn a_prompt_with_no_choice_ends_the_action_quietly(reason: &str) {
    let rex = Rex::load("ui");
    rex.script(&[Answer::Unanswered(reason, None)]);
    assert_eq!(rex.run(json!({})), Ok(json!(null)));
    assert_eq!(rex.kinds(), ["exec", "pick"], "no tab and no layer opens");
}

#[test_case("no_client", None, "no Rex app is connected to show the host list\nFix: open the Rex app, then run the action again."; "no app")]
#[test_case("busy", None, "Rex is showing another prompt\nFix: answer or close it, then run the action again."; "another prompt is open")]
#[test_case("failed", Some("boom"), "Rex could not show the host list: boom"; "failed, with its detail")]
#[test_case("failed", None, "Rex could not show the host list"; "failed, with no detail")]
#[test_case("later", None, "Rex gave no answer to the host list: later"; "a reason Rex adds later")]
fn a_prompt_rex_cannot_show_is_an_action_error(reason: &str, detail: Option<&str>, want: &str) {
    let rex = Rex::load("ui");
    rex.script(&[Answer::Unanswered(reason, detail)]);
    assert_eq!(rex.run(json!({})), Err(want.to_owned()));
    assert_eq!(rex.kinds(), ["exec", "pick"], "no tab and no layer opens");
}

#[test]
fn a_config_without_hosts_is_sshpicks_error_and_no_prompt_opens() {
    let rex = Rex::load("empty");
    assert_eq!(
        rex.run(json!({})),
        Err("no hosts found in ~/.ssh/config or the files it Includes\n\
             Fix: add `Host` lines; ENV and WHAT come from comments."
            .to_owned())
    );
    assert_eq!(rex.kinds(), ["exec"]);
}

#[test]
fn a_config_warning_is_logged_and_the_hosts_are_still_listed() {
    let rex = Rex::load("warns");
    rex.script(&[CANCELLED]);
    assert_eq!(rex.run(json!({})), Ok(json!(null)));
    assert_eq!(rex.kinds(), ["exec", "log", "pick"]);
    let log = rex.call("log");
    assert_eq!(log["level"], "warn");
    assert!(
        log["message"]
            .as_str()
            .or_panic()
            .ends_with(": skipped, include cycle"),
        "{log}"
    );
    assert_eq!(rex.prompts()[0].1, ["warm-a", "warm-b"]);
}

/// A home whose config has `count` hosts, `host-0000` on, written under cargo's test directory.
fn home_with_hosts(count: usize) -> String {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("rex-hosts-{}", process::id()));
    let ssh = home.join(".ssh");
    std::fs::create_dir_all(&ssh).or_panic();
    let config: String = (0..count).map(|n| format!("Host host-{n:04}\n")).collect();
    std::fs::write(ssh.join("config"), config).or_panic();
    home.display().to_string()
}

#[test]
fn more_hosts_than_rex_takes_are_cut_and_the_title_says_so() {
    let rex = Rex::load(&home_with_hosts(2345));
    rex.script(&[Answer::Text("host-"), Answer::Text("host-1"), CANCELLED]);
    assert_eq!(rex.run(json!({})), Ok(json!(null)));
    let prompts = rex.prompts();
    let titles: Vec<&str> = prompts.iter().map(|(title, _)| title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "First 2,000 of 2,345 hosts",
            "First 2,000 of 2,345 hosts match \"host-\"",
            "1,000 hosts match \"host-1\"",
        ]
    );
    for (_, ids) in &prompts[..2] {
        assert_eq!(ids.len(), 2000);
        assert_eq!(ids.first().map(String::as_str), Some("host-0000"));
        assert_eq!(ids.last().map(String::as_str), Some("host-1999"));
    }
    assert_eq!(prompts[2].1.len(), 1000);
}

#[test]
fn a_rex_app_too_old_for_prompts_gets_a_focused_layer_that_runs_the_hand_off_with_both_binaries() {
    let rex = Rex::load("ui");
    rex.script(&[Answer::Unanswered("unsupported", None)]);
    assert_eq!(rex.run(json!({})), Ok(json!(null)));
    let layer = rex.call("new_layer")["args"].clone();
    assert_eq!(layer["bounds"], json!([0.1, 0.1, 0.8, 0.8]));
    assert_eq!(layer["focus"], true);
    let block = &layer["layout"]["block"];
    assert_eq!(block["flavor"], TERMINAL);
    assert_eq!(block["label"], "sshpick");
    assert_eq!(block["options"]["shell"], "none");
    let argv: Vec<String> = serde_json::from_value(block["options"]["command"].clone()).or_panic();
    assert_eq!(argv[..2], ["/bin/sh", "-c"]);
    assert_eq!(argv[3..], ["sshpick-rex", SSHPICK, &fake("rex")]);
    assert_eq!(rex.kinds(), ["exec", "pick", "new_layer"]);
}

struct Handoff {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    /// The arguments rex was called with, or `None` when it was not called.
    rex: Option<Vec<String>>,
}

/// The hand-off argv run through pipes, with `enter` on stdin for the "Press Enter" wait.
fn run_handoff(argv: &[String], home: &str, env: &[(&str, &str)], name: &str) -> Handoff {
    let record = scratch(name);
    let mut child = process::Command::new(&argv[0])
        .args(&argv[1..])
        .env("HOME", fixtures().join(home))
        .env("PATH", search_path())
        .env("REX_RECORD", &record)
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .or_panic();
    // The hand-off may exit without reading it, closing the pipe first.
    let _ = child.stdin.take().or_panic().write_all(b"\n");
    let output = child.wait_with_output().or_panic();
    let rex = std::fs::read_to_string(&record)
        .ok()
        .map(|text| text.lines().map(str::to_owned).collect());
    let _ = std::fs::remove_file(&record);
    Handoff {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        rex,
    }
}

#[test]
fn the_hand_off_gives_the_picked_host_back_to_the_action() {
    let rex = Rex::load("home");
    let argv = rex.handoff(SSHPICK);
    let done = run_handoff(&argv, "home", &[], "picked");
    assert_eq!(done.code, Some(0), "{}", done.stderr);
    assert_eq!(done.stdout, "");
    let called = done.rex.or_panic();
    assert_eq!(called, ["do", "sshpick", "host=only-host"]);

    // `rex do sshpick host=...` runs the action again, with the host.
    let again = Rex::load("home");
    again.run(json!({ "host": "only-host" })).or_panic();
    assert_eq!(
        again.call("new_window")["args"]["layout"]["block"]["options"]["command"],
        session_argv("only-host")
    );
}

#[test_case("130", Some(0), false; "esc closes quietly")]
#[test_case("1", Some(0), false; "enter on no match closes quietly")]
#[test_case("2", Some(2), true; "any other failure waits for enter")]
#[test_case("127", Some(127), true; "a missing ssh waits too")]
fn the_hand_off_closes_quietly_only_on_a_cancel(exit: &str, code: Option<i32>, waits: bool) {
    let rex = Rex::load("ui");
    let argv = rex.handoff(&fake("sshpick-exits"));
    let done = run_handoff(&argv, "ui", &[("FAKE_EXIT", exit)], &format!("exit-{exit}"));
    assert_eq!(done.code, code, "{}", done.stderr);
    assert_eq!(done.rex, None, "rex is not called");
    assert_eq!(
        done.stdout.contains("Press Enter to close."),
        waits,
        "{}",
        done.stdout
    );
    assert!(
        done.stderr.contains("sshpick-exits --print"),
        "{}",
        done.stderr
    );
}

#[test]
fn the_hand_off_waits_when_sshpick_is_too_old_for_print() {
    let rex = Rex::load("ui");
    let done = run_handoff(&rex.handoff(&fake("sshpick-old")), "ui", &[], "old");
    assert_eq!(done.code, Some(2));
    assert!(
        done.stderr.contains("unknown option \"--print\""),
        "{}",
        done.stderr
    );
    assert!(done.stdout.contains("Press Enter to close."));
    assert_eq!(done.rex, None);
}

#[test]
fn the_hand_off_waits_when_rex_refuses_the_host() {
    let rex = Rex::load("home");
    let done = run_handoff(
        &rex.handoff(SSHPICK),
        "home",
        &[("REX_EXIT", "3")],
        "refused",
    );
    assert_eq!(done.code, Some(3));
    assert!(
        done.stdout.contains("Press Enter to close."),
        "{}",
        done.stdout
    );
    assert!(done.rex.is_some());
}

#[test_case("255", Some(255), true; "ssh failing to connect waits for enter")]
#[test_case("0", Some(0), false; "a logout closes the tab")]
#[test_case("130", Some(130), false; "a remote status passes through")]
fn the_tab_holds_only_when_ssh_itself_fails(exit: &str, code: Option<i32>, waits: bool) {
    let argv: Vec<String> = serde_json::from_value(session_argv("web")).or_panic();
    let mut argv = argv;
    argv[4] = fake("sshpick-exits");
    let done = run_handoff(&argv, "ui", &[("FAKE_EXIT", exit)], &format!("tab-{exit}"));
    assert_eq!(done.code, code, "{}", done.stderr);
    assert_eq!(
        done.stdout.contains("Press Enter to close."),
        waits,
        "{}",
        done.stdout
    );
    assert!(
        done.stderr
            .contains("sshpick-exits --non-interactive -- web"),
        "{}",
        done.stderr
    );
}

/// The layer as the owner sees it: sshpick's picker drawn on the layer's terminal, a query, Enter.
fn picker_layer(keys: &str, name: &str) -> (Exit, Option<Vec<String>>) {
    let rex = Rex::load("ui");
    let record = scratch(name);
    let mut options = Options::new("ui");
    options.program = Some(rex.handoff(SSHPICK));
    options.env = vec![("REX_RECORD".to_owned(), record.display().to_string())];
    let mut session = Session::start(options);
    session.wait_for("the picker", |frame| {
        frame.alternate && frame.rows[23].contains("esc quit")
    });
    session.send(keys);
    if keys.ends_with('\r') {
        session.wait_for("the picker to close", |frame| !frame.alternate);
    }
    let exit = session.exit();
    let called = std::fs::read_to_string(&record)
        .ok()
        .map(|text| text.lines().map(str::to_owned).collect());
    let _ = std::fs::remove_file(&record);
    (exit, called)
}

#[test]
fn enter_in_the_picker_layer_hands_the_host_to_rex() {
    let (exit, called) = picker_layer("lab-a\r", "pty-enter");
    assert_eq!(exit, Exit::Code(0));
    let called = called.or_panic();
    assert_eq!(called, ["do", "sshpick", "host=lab-a"]);
}

#[test]
fn esc_in_the_picker_layer_closes_it_without_calling_rex() {
    let (exit, called) = picker_layer("\x1b", "pty-esc");
    assert_eq!(exit, Exit::Code(0));
    assert_eq!(called, None);
}

#[test]
fn the_docs_show_the_lines_the_tests_load() {
    let docs = include_str!("../../../docs/rex.md");
    for line in [
        "require(\"sshpick\")",
        "rex.bind(\"ctrl+a>s\", \"sshpick\")",
        "rex do sshpick host=",
        "integrations/rex/sshpick.lua",
    ] {
        assert!(docs.contains(line), "docs/rex.md lacks {line}");
    }
    assert!(include_str!("../../../docs/README.md").contains("(rex.md)"));
    assert!(include_str!("../../../README.md").contains("docs/rex.md"));
}
