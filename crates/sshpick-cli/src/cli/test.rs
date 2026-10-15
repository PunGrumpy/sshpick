use std::ffi::OsString;
use std::io;

use insta::assert_snapshot;
use sshpick_ui::ColorConfig;
use test_case::test_case;

use super::args::{self, Command, UsageError, parse};
use super::error::{Error, Exit};
use super::help::{self, MAX_WIDTH};
use super::spec::{self, COMMANDS, GLOBAL};
use crate::commands::{inspect, list, pick};

const PLAIN: ColorConfig = ColorConfig::none();

fn argv(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn command(args: &[&str]) -> Result<Command, UsageError> {
    parse(argv(args)).map(|parsed| parsed.command)
}

fn pick_for(query: &str) -> Result<Command, UsageError> {
    Ok(Command::Pick {
        query: query.to_owned(),
    })
}

fn list_for(query: &str, machine: bool) -> Result<Command, UsageError> {
    Ok(Command::List {
        query: query.to_owned(),
        json: false,
        machine,
    })
}

fn list_json(query: &str) -> Result<Command, UsageError> {
    Ok(Command::List {
        query: query.to_owned(),
        json: true,
        machine: false,
    })
}

fn inspect_for(query: &str, json: bool) -> Result<Command, UsageError> {
    Ok(Command::Inspect {
        query: query.to_owned(),
        json,
    })
}

fn help_for(topic: Option<&'static str>) -> Result<Command, UsageError> {
    Ok(Command::Help { topic })
}

fn unknown_option(option: &str) -> Result<Command, UsageError> {
    unknown_option_of(None, option)
}

fn unknown_option_of(command: Option<&'static str>, option: &str) -> Result<Command, UsageError> {
    Err(UsageError::Option {
        option: option.to_owned(),
        command,
    })
}

fn unknown_command(word: &str) -> Result<Command, UsageError> {
    Err(UsageError::Command(word.to_owned()))
}

#[test_case(&[] => pick_for(""); "no arguments open the picker")]
#[test_case(&["new", "consul"] => pick_for("new consul"); "words join into one query")]
#[test_case(&["pick"] => pick_for(""); "pick by name")]
#[test_case(&["pick", "list"] => pick_for("list"); "pick makes a command word a query")]
#[test_case(&["--", "list"] => pick_for("list"); "after -- a command word is a query")]
#[test_case(&["--", "-h"] => pick_for("-h"); "after -- an option is query text")]
#[test_case(&["a", "--", "-h"] => pick_for("a -h"); "words before and after --")]
#[test_case(&["--", "--", "-l"] => pick_for("-- -l"); "a second -- is query text")]
#[test_case(&["--"] => pick_for(""); "a bare --")]
#[test_case(&["-"] => pick_for("-"); "a lone dash is query text")]
#[test_case(&["db", "list"] => pick_for("db list"); "a command word later on is a query")]
#[test_case(&["lst"] => pick_for("lst"); "a misspelt command is a query")]
#[test_case(&["inspect", "stg-consul1"] => inspect_for("stg-consul1", false); "inspect a host")]
#[test_case(&["inspect", "new", "consul"] => inspect_for("new consul", false); "inspect takes the whole query")]
#[test_case(&["inspect", "--", "-x"] => inspect_for("-x", false); "inspect a dashed query")]
#[test_case(&["inspect", "x", "--json"] => inspect_for("x", true); "inspect as json")]
#[test_case(&["--json", "inspect", "x"] => inspect_for("x", true); "json before the command")]
#[test_case(&["inspect"] => Err(UsageError::MissingQuery("inspect")); "inspect needs a query")]
#[test_case(&["inspekt", "x"] => pick_for("inspekt x"); "a misspelt command is a query")]
#[test_case(&["pick", "inspect"] => pick_for("inspect"); "pick makes inspect a query")]
#[test_case(&["list", "--json"] => list_json(""); "list as json")]
#[test_case(&["--json", "ls", "new"] => list_json("new"); "ls as json")]
#[test_case(&["list"] => list_for("", false); "list")]
#[test_case(&["ls", "new", "consul"] => list_for("new consul", false); "ls with a query")]
#[test_case(&["list", "--", "-x"] => list_for("-x", false); "list with a dashed query")]
#[test_case(&["db", "--list"] => list_for("db", true); "long list after a word")]
#[test_case(&["-l", "db"] => list_for("db", true); "short list with a query")]
#[test_case(&["-l"] => list_for("", true); "list with no query")]
#[test_case(&["--list", "--", "-x", "y"] => list_for("-x y", true); "list with a dashed query after --")]
#[test_case(&["ls", "-l"] => list_for("", true); "ls with -l is machine mode")]
#[test_case(&["pick", "-l", "db"] => list_for("db", true); "pick with -l lists")]
#[test_case(&["-h"] => help_for(None); "short help")]
#[test_case(&["--help"] => help_for(None); "long help")]
#[test_case(&["db", "-h"] => help_for(None); "help after a word")]
#[test_case(&["ls", "--help"] => help_for(Some("list")); "help for ls")]
#[test_case(&["list", "-h"] => help_for(Some("list")); "help for list")]
#[test_case(&["pick", "--help"] => help_for(Some("pick")); "help for pick")]
#[test_case(&["--help", "ls"] => help_for(Some("list")); "help before the command")]
#[test_case(&["help"] => help_for(None); "the help command")]
#[test_case(&["help", "ls"] => help_for(Some("list")); "help ls")]
#[test_case(&["help", "list"] => help_for(Some("list")); "help list")]
#[test_case(&["help", "--help"] => help_for(Some("help")); "help for help")]
#[test_case(&["help", "lst"] => unknown_command("lst"); "help for an unknown command")]
#[test_case(&["--bogus", "-h"] => help_for(None); "help beats an earlier unknown option")]
#[test_case(&["ls", "--bogus", "--help"] => help_for(Some("list")); "help beats an unknown option")]
#[test_case(&["-h", "--bogus"] => help_for(None); "help first")]
#[test_case(&["--bogus"] => unknown_option("--bogus"); "an unknown long option")]
#[test_case(&["-lh"] => unknown_option("-lh"); "clustered short options are unknown")]
#[test_case(&["-l", "-x"] => unknown_option("-x"); "an unknown option after -l")]
#[test_case(&["--bogus", "-x"] => unknown_option("--bogus"); "the first unknown option is named")]
#[test_case(&["--json"] => unknown_option("--json"); "json needs a command that accepts it")]
#[test_case(&["pick", "--json"] => unknown_option_of(Some("pick"), "--json"); "pick has no json")]
#[test_case(&["help", "--json"] => unknown_option_of(Some("help"), "--json"); "help has no json")]
#[test_case(&["pick", "--bogus"] => unknown_option_of(Some("pick"), "--bogus"); "the command is remembered")]
#[test_case(&["--version"] => Ok(Command::Version); "long version")]
#[test_case(&["-V"] => Ok(Command::Version); "short version")]
#[test_case(&["-v"] => Ok(Command::Version); "hidden lowercase version")]
#[test_case(&["db", "-V"] => Ok(Command::Version); "version after a query word")]
#[test_case(&["ls", "-V"] => list_for("", false); "version is ignored when a command is named")]
#[test_case(&["--no-color", "db"] => pick_for("db"); "no-color is a flag, not a word")]
#[test_case(&["--non-interactive", "db"] => pick_for("db"); "non-interactive is a flag, not a word")]
fn argv_selects_a_command(args: &[&str]) -> Result<Command, UsageError> {
    command(args)
}

#[test_case(&[] => None; "no command word")]
#[test_case(&["db"] => None; "a query word")]
#[test_case(&["list"] => Some("list".to_owned()); "list as typed")]
#[test_case(&["ls", "x"] => Some("ls".to_owned()); "ls as typed")]
#[test_case(&["inspect", "x"] => Some("inspect".to_owned()); "inspect as typed")]
#[test_case(&["x", "--", "list"] => None; "after the double dash")]
fn the_command_word_is_kept_as_typed(args: &[&str]) -> Option<String> {
    parse(argv(args)).unwrap().typed
}

#[test]
fn global_flags_are_read_wherever_they_stand() {
    let global = |args: &[&str]| parse(argv(args)).unwrap().global;
    assert!(global(&["--non-interactive"]).non_interactive);
    assert!(global(&["ls", "--non-interactive"]).non_interactive);
    assert!(!global(&["--", "--non-interactive"]).non_interactive);
    assert!(!global(&["db"]).non_interactive);
    assert!(global(&["db", "--no-color"]).no_color);
}

#[test]
fn no_color_is_read_before_the_strict_parse() {
    assert!(parse(argv(&["--no-color"])).unwrap().global.no_color);
    assert!(!parse(argv(&["db"])).unwrap().global.no_color);
    assert!(args::wants_no_color(&argv(&["--bogus", "--no-color"])));
    assert!(args::wants_no_color(&argv(&["ls", "--no-color"])));
    assert!(!args::wants_no_color(&argv(&["--", "--no-color"])));
    assert!(!args::wants_no_color(&argv(&["--no-colour"])));
}

#[test]
fn the_table_has_no_two_commands_with_the_same_name() {
    let mut words: Vec<&str> = COMMANDS
        .iter()
        .flat_map(|spec| spec.aliases.iter().copied().chain([spec.name]))
        .collect();
    let count = words.len();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), count);
    assert_eq!(COMMANDS.iter().filter(|spec| spec.default).count(), 1);
    assert_eq!(spec::command("ls").map(|spec| spec.name), Some("list"));
    assert!(spec::command("lst").is_none());
}

#[test_case(Error::Usage(UsageError::Command("x".to_owned())), 2; "unknown command")]
#[test_case(Error::Usage(UsageError::Option { option: "--x".to_owned(), command: None }), 2; "usage")]
#[test_case(no_match("x"), 1; "no match")]
#[test_case(Error::Usage(UsageError::MissingQuery("inspect")), 2; "missing query")]
#[test_case(Error::Inspect(inspect::Error::Several { count: 2, query: "x".to_owned(), names: vec![] }), 2; "inspect several")]
#[test_case(Error::Inspect(inspect::Error::Resolve(sshpick_core::ResolveError::Timeout)), 2; "ssh -G failed")]
#[test_case(Error::Inspect(inspect::Error::Write(io::Error::other("x"))), 2; "inspect write")]
#[test_case(Error::Pick(several(4, pick::Refusal::NoTerminal)), 2; "several matches")]
#[test_case(Error::Stdout(io::Error::other("x")), 2; "stdout")]
#[test_case(Error::NoHome, 2; "no home")]
#[test_case(Error::NoHosts, 2; "no hosts")]
#[test_case(Error::List(list::Error::Write(io::Error::other("x"))), 2; "list write")]
#[test_case(Error::Pick(pick::Error::Previews(io::Error::other("x"))), 2; "previews")]
#[test_case(Error::Pick(pick::Error::Terminal(io::Error::other("x"))), 2; "terminal")]
#[test_case(Error::Pick(pick::Error::Exec(io::Error::other("x"))), 127; "exec failed")]
fn errors_map_to_the_documented_exit_codes(error: Error, code: u8) {
    assert_eq!(error.exit_code(), code);
}

#[test]
fn exit_codes_are_the_documented_ones() {
    assert_eq!(Exit::Success.code(), 0);
    assert_eq!(Exit::NoMatch.code(), 1);
    assert_eq!(Exit::Error.code(), 2);
    assert_eq!(Exit::ExecFailed.code(), 127);
    assert_eq!(Exit::Cancelled.code(), 130);
    let root = help::render(None, 80, PLAIN);
    for code in ["0 ", "1 ", "2 ", "127 ", "130 "] {
        assert!(
            root.lines().any(|line| line.trim_start().starts_with(code)),
            "the root help lacks exit code {code}"
        );
    }
}

fn page(topic: &str, width: usize) -> String {
    help::render(spec::command(topic), width, PLAIN)
}

#[test]
fn the_root_help() {
    assert_snapshot!("help_root", help::render(None, 80, PLAIN));
}

#[test_case("pick")]
#[test_case("list")]
#[test_case("inspect")]
#[test_case("help")]
fn each_commands_help(topic: &str) {
    assert_snapshot!(format!("help_{topic}"), page(topic, 80));
}

#[test_case(40)]
#[test_case(100)]
fn the_root_help_wraps_to_the_width(width: usize) {
    assert_snapshot!(
        format!("help_root_{width}"),
        help::render(None, width, PLAIN)
    );
}

#[test]
fn help_for_ls_and_list_is_the_same_page() {
    assert_eq!(page("ls", 80), page("list", 80));
}

#[test]
fn help_leads_with_the_banner_that_version_prints() {
    let banner = format!("sshpick {}\n\n", env!("CARGO_PKG_VERSION"));
    assert!(help::render(None, 80, PLAIN).starts_with(&banner));
}

#[test]
fn no_help_line_ends_in_a_space_at_any_width() {
    let topics = std::iter::once(None).chain(COMMANDS.iter().copied().map(Some));
    for topic in topics {
        for width in 0..=140 {
            let text = help::render(topic, width, PLAIN);
            for line in text.lines() {
                assert!(!line.ends_with(' '), "{width}: {line:?}");
            }
            assert!(text.ends_with('\n'));
            assert!(!text.ends_with("\n\n"), "{width}: one newline at the end");
        }
    }
}

#[test]
fn help_fits_a_100_column_terminal() {
    let topics = std::iter::once(None).chain(COMMANDS.iter().copied().map(Some));
    for topic in topics {
        for width in [80, MAX_WIDTH, 300] {
            for line in help::render(topic, width, PLAIN).lines() {
                assert!(line.chars().count() <= MAX_WIDTH, "{line}");
            }
        }
    }
}

#[test]
fn hidden_options_parse_but_stay_out_of_help() {
    let hidden: Vec<&str> = GLOBAL
        .iter()
        .filter(|option| option.description.is_none())
        .map(|option| option.long)
        .collect();
    assert_eq!(hidden, ["list"]);
    let topics = std::iter::once(None).chain(COMMANDS.iter().copied().map(Some));
    for topic in topics {
        let text = help::render(topic, 80, PLAIN);
        assert!(!text.contains("--list"), "{text}");
        assert_eq!(
            text.contains("--json"),
            topic.is_some_and(|spec| !spec.options.is_empty())
        );
        assert!(!text.contains(" -v"), "{text}");
    }
}

fn strip_escapes(text: &str) -> String {
    let mut plain = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for end in chars.by_ref() {
                if end == 'm' {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain
}

#[test]
fn colour_changes_the_styling_and_nothing_else() {
    let colors = ColorConfig::new(true, false, &|_| None);
    for topic in [None, spec::command("pick"), spec::command("list")] {
        let styled = help::render(topic, 80, colors);
        assert_eq!(strip_escapes(&styled), help::render(topic, 80, PLAIN));
        assert!(styled.contains("\x1b[2mGlobal Options\x1b[22m:"));
        assert!(styled.contains("\x1b[90m-\x1b[39m"));
        assert!(styled.contains("\x1b[36m$ sshpick"));
        assert!(styled.contains("\x1b[1msshpick\x1b[22m"));
    }
    let root = help::render(None, 80, colors);
    assert!(root.contains("\x1b[1m(default)\x1b[22m"));
    let pick = help::render(spec::command("pick"), 80, colors);
    assert!(pick.contains("\x1b[1msshpick\x1b[22m \x1b[1mpick\x1b[22m"));
}

#[test]
fn a_stream_without_colour_gets_no_escape_bytes() {
    let no_hue = ColorConfig::new(false, true, &|_| None);
    for topic in [None, spec::command("list")] {
        assert!(!help::render(topic, 80, no_hue).contains('\x1b'));
    }
}

fn no_match(query: &str) -> Error {
    Error::NoMatch {
        query: query.to_owned(),
        did_you_mean: None,
    }
}

fn several(count: usize, refusal: pick::Refusal) -> pick::Error {
    pick::Error::Several {
        count,
        query: "consul".to_owned(),
        refusal,
    }
}

fn shown(error: &Error) -> String {
    sshpick_ui::error_text(PLAIN, error, error.fix().as_deref(), error.docs())
}

#[test]
fn every_error_is_an_error_line_then_an_optional_fix_and_learn_more() {
    let option = |option: &str, command| {
        Error::Usage(UsageError::Option {
            option: option.to_owned(),
            command,
        })
    };
    let load = sshpick_core::load(std::path::Path::new("/nonexistent-home")).unwrap_err();
    let errors = [
        ("sshpick --lst", option("--lst", None)),
        ("sshpick list --lsit", option("--lsit", Some("list"))),
        ("sshpick --bogus", option("--bogus", None)),
        (
            "sshpick help lst",
            Error::Usage(UsageError::Command("lst".to_owned())),
        ),
        (
            "sshpick help bogus",
            Error::Usage(UsageError::Command("bogus".to_owned())),
        ),
        (
            "sshpick consul < /dev/null",
            Error::Pick(several(4, pick::Refusal::NoTerminal)),
        ),
        (
            "sshpick --non-interactive",
            Error::Pick(pick::Error::Several {
                count: 12,
                query: String::new(),
                refusal: pick::Refusal::NonInteractive,
            }),
        ),
        (
            "sshpick lst new < /dev/null",
            Error::NoMatch {
                query: "lst new".to_owned(),
                did_you_mean: Some("list new".to_owned()),
            },
        ),
        ("sshpick nope < /dev/null", no_match("nope")),
        (
            "sshpick inspekt stg-consul1 < /dev/null",
            Error::NoMatch {
                query: "inspekt stg-consul1".to_owned(),
                did_you_mean: Some("inspect stg-consul1".to_owned()),
            },
        ),
        (
            "sshpick inspect",
            Error::Usage(UsageError::MissingQuery("inspect")),
        ),
        (
            "sshpick inspect consul",
            Error::Inspect(inspect::Error::Several {
                count: 3,
                query: "consul".to_owned(),
                names: ["stg-consul1", "stg-consul2", "stg-consul3"]
                    .map(str::to_owned)
                    .to_vec(),
            }),
        ),
        (
            "sshpick inspect web",
            Error::Inspect(inspect::Error::Several {
                count: 8,
                query: "web".to_owned(),
                names: (1..=8).map(|n| format!("web{n}")).collect(),
            }),
        ),
        (
            "sshpick inspect stg-broken",
            Error::Inspect(inspect::Error::Resolve(sshpick_core::ResolveError::Ssh {
                code: Some(255),
                stderr: "/home/me/.ssh/config: terminating, 1 bad configuration options".to_owned(),
            })),
        ),
        (
            "sshpick inspect stg-slow",
            Error::Inspect(inspect::Error::Resolve(sshpick_core::ResolveError::Timeout)),
        ),
        ("sshpick  # no Host lines", Error::NoHosts),
        ("sshpick  # no ~/.ssh/config", Error::Load(load)),
        ("sshpick  # no HOME", Error::NoHome),
        (
            "sshpick stg-consul1  # no ssh",
            Error::Pick(pick::Error::Exec(io::Error::from_raw_os_error(2))),
        ),
        (
            "sshpick stg-consul1  # ssh refused",
            Error::Pick(pick::Error::Exec(io::Error::from_raw_os_error(13))),
        ),
        (
            "sshpick ls  # stdout is full",
            Error::List(list::Error::Write(io::Error::from_raw_os_error(28))),
        ),
    ];
    let mut text = String::new();
    for (command, error) in &errors {
        let block = shown(error);
        assert!(!block.contains("sshpick:"), "{block}");
        assert!(block.starts_with("Error: "), "{block}");
        text.push_str(&format!("$ {command}\n{block}\n"));
    }
    assert_snapshot!("errors", text);
}

#[test]
fn colour_is_only_in_the_labels() {
    let colors = ColorConfig::new(true, false, &|_| None);
    let error = Error::NoHosts;
    let text = sshpick_ui::error_text(colors, &error, error.fix().as_deref(), error.docs());
    assert_eq!(strip_escapes(&text), shown(&error));
    assert!(text.starts_with("\x1b[31mError:\x1b[39m no hosts found"));
    assert!(text.contains("\x1b[1mFix:\x1b[22m add `Host` lines"));
    assert!(text.contains("\x1b[1mLearn More:\x1b[22m https://"));
}
