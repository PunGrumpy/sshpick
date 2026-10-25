use std::ffi::OsString;
use std::io;

use insta::assert_snapshot;
use sshpick_core::Refusal;
use sshpick_ui::ColorConfig;
use test_case::test_case;

use super::args::{Command, UsageError, parse};
use super::error::{Error, Exit};
use super::help::{self, MAX_WIDTH};
use super::spec::{self, COMMANDS, EXIT_STATUS, GLOBAL};
use crate::commands::list::ListFormat;
use crate::commands::{doctor, inspect, list, pick};

const PLAIN: ColorConfig = ColorConfig::none();

fn argv(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn command(args: &[&str]) -> Result<Command, UsageError> {
    parse(argv(args)).command
}

fn pick_for(query: &str) -> Result<Command, UsageError> {
    Ok(Command::Pick {
        query: query.to_owned(),
    })
}

fn list_as(query: &str, format: ListFormat) -> Result<Command, UsageError> {
    Ok(Command::List {
        query: query.to_owned(),
        format,
    })
}

fn list_json(query: &str) -> Result<Command, UsageError> {
    list_as(query, ListFormat::Json)
}

fn inspect_for(query: &str, json: bool) -> Result<Command, UsageError> {
    Ok(Command::Inspect {
        query: query.to_owned(),
        json,
    })
}

fn doctor_for(json: bool) -> Result<Command, UsageError> {
    Ok(Command::Doctor { json })
}

fn unexpected(command: &'static str, word: &str) -> Result<Command, UsageError> {
    Err(UsageError::Unexpected {
        command,
        word: word.to_owned(),
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
#[test_case(&["doctor"] => doctor_for(false); "doctor")]
#[test_case(&["doctor", "--json"] => doctor_for(true); "doctor as json")]
#[test_case(&["--json", "doctor"] => doctor_for(true); "json before the command")]
#[test_case(&["doctor", "-l"] => doctor_for(false); "doctor ignores --list like inspect does")]
#[test_case(&["doctor", "extra"] => unexpected("doctor", "extra"); "doctor takes no arguments")]
#[test_case(&["doctor", "a", "b"] => unexpected("doctor", "a"); "the first extra word is named")]
#[test_case(&["doctor", "--", "extra"] => unexpected("doctor", "extra"); "doctor takes no query after -- either")]
#[test_case(&["doctor", "--bogus"] => unknown_option_of(Some("doctor"), "--bogus"); "doctor remembers itself in an unknown option")]
#[test_case(&["pick", "doctor"] => pick_for("doctor"); "pick makes doctor a query")]
#[test_case(&["--", "doctor"] => pick_for("doctor"); "after -- doctor is a query")]
#[test_case(&["db", "doctor"] => pick_for("db doctor"); "a doctor later on is a query")]
#[test_case(&["doctr"] => pick_for("doctr"); "a misspelt doctor is a query")]
#[test_case(&["list", "--json"] => list_json(""); "list as json")]
#[test_case(&["--json", "ls", "new"] => list_json("new"); "ls as json")]
#[test_case(&["list", "--list", "--json"] => list_json(""); "json beats --list on list")]
#[test_case(&["ls", "--json", "-l"] => list_json(""); "json beats -l whichever comes first")]
#[test_case(&["list"] => list_as("", ListFormat::Auto); "list")]
#[test_case(&["ls", "new", "consul"] => list_as("new consul", ListFormat::Auto); "ls with a query")]
#[test_case(&["list", "--", "-x"] => list_as("-x", ListFormat::Auto); "list with a dashed query")]
#[test_case(&["db", "--list"] => list_as("db", ListFormat::Tsv); "long list after a word")]
#[test_case(&["-l", "db"] => list_as("db", ListFormat::Tsv); "short list with a query")]
#[test_case(&["-l"] => list_as("", ListFormat::Tsv); "list with no query")]
#[test_case(&["--list", "--", "-x", "y"] => list_as("-x y", ListFormat::Tsv); "list with a dashed query after --")]
#[test_case(&["ls", "-l"] => list_as("", ListFormat::Tsv); "ls with -l is tab-separated")]
#[test_case(&["pick", "-l", "db"] => list_as("db", ListFormat::Tsv); "pick with -l lists")]
#[test_case(&["-h"] => help_for(None); "short help")]
#[test_case(&["--help"] => help_for(None); "long help")]
#[test_case(&["db", "-h"] => help_for(None); "help after a word")]
#[test_case(&["ls", "--help"] => help_for(Some("list")); "help for ls")]
#[test_case(&["list", "-h"] => help_for(Some("list")); "help for list")]
#[test_case(&["pick", "--help"] => help_for(Some("pick")); "help for pick")]
#[test_case(&["--help", "ls"] => help_for(Some("list")); "help before the command")]
#[test_case(&["help"] => help_for(None); "the help command")]
#[test_case(&["help", "ls"] => help_for(Some("list")); "help ls")]
#[test_case(&["help", "doctor"] => help_for(Some("doctor")); "help doctor")]
#[test_case(&["doctor", "--help"] => help_for(Some("doctor")); "help for doctor")]
#[test_case(&["doctor", "extra", "-h"] => help_for(Some("doctor")); "help beats an extra argument")]
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
#[test_case(&["ls", "-V"] => list_as("", ListFormat::Auto); "version is ignored when a command is named")]
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
#[test_case(&["doctor"] => Some("doctor".to_owned()); "doctor as typed")]
#[test_case(&["x", "--", "list"] => None; "after the double dash")]
fn the_command_word_is_kept_as_typed(args: &[&str]) -> Option<String> {
    parse(argv(args)).typed
}

#[test]
fn global_flags_are_read_wherever_they_stand() {
    let global = |args: &[&str]| parse(argv(args)).global;
    assert!(global(&["--non-interactive"]).non_interactive);
    assert!(global(&["ls", "--non-interactive"]).non_interactive);
    assert!(!global(&["--", "--non-interactive"]).non_interactive);
    assert!(!global(&["db"]).non_interactive);
    assert!(global(&["db", "--no-color"]).no_color);
}

#[test_case(&["--no-color"], true; "alone")]
#[test_case(&["ls", "--no-color"], true; "after a command")]
#[test_case(&["--bogus", "--no-color"], true; "beside an unknown option")]
#[test_case(&["--no-color", "--bogus"], true; "before an unknown option")]
#[test_case(&["inspect", "--no-color"], true; "beside a missing query")]
#[test_case(&["--", "--no-color"], false; "after -- it is query text")]
#[test_case(&["--bogus", "--", "--no-color"], false; "after -- on a failed parse")]
#[test_case(&["--no-colour"], false; "only the exact spelling")]
#[test_case(&["db"], false; "absent")]
fn no_color_is_read_whether_or_not_the_command_line_parses(args: &[&str], want: bool) {
    assert_eq!(parse(argv(args)).global.no_color, want, "{args:?}");
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
#[test_case(Error::Usage(UsageError::Unexpected { command: "doctor", word: "x".to_owned() }), 2; "unexpected argument")]
#[test_case(Error::Doctor(doctor::Error::Write(io::Error::other("x"))), 2; "doctor write")]
#[test_case(Error::Inspect(inspect::Error::Several { count: 2, query: "x".to_owned(), names: vec![] }), 2; "inspect several")]
#[test_case(Error::Inspect(inspect::Error::Resolve(sshpick_core::ResolveError::Timeout)), 2; "ssh -G failed")]
#[test_case(Error::Inspect(inspect::Error::Write(io::Error::other("x"))), 2; "inspect write")]
#[test_case(Error::Pick(several(4, Refusal::NoTerminal)), 2; "several matches")]
#[test_case(Error::Pick(several(4, Refusal::DumbTerminal)), 2; "several matches on a dumb terminal")]
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

#[test]
fn the_docs_list_every_command_word() {
    let docs = include_str!("../../../../docs/commands.md");
    let sentence: String = docs
        .lines()
        .skip_while(|line| !line.contains("are command words when they come first"))
        .take_while(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    for (word, _) in spec::command_words() {
        assert!(
            sentence.contains(&format!("`{word}`")),
            "the command-words sentence in docs/commands.md lacks `{word}`"
        );
    }
    for spec in COMMANDS {
        let row = docs.lines().any(|line| {
            line.starts_with('|')
                && line
                    .split('|')
                    .nth(1)
                    .is_some_and(|cell| cell.contains(&format!("`{}", spec.name)))
        });
        assert!(
            row,
            "the command table in docs/commands.md lacks {}",
            spec.name
        );
    }
}

#[test]
fn the_docs_exit_table_has_the_codes_of_the_help_and_names_a_failed_doctor_check() {
    let docs = include_str!("../../../../docs/scripting.md");
    let table: Vec<&str> = docs
        .lines()
        .skip_while(|line| !line.starts_with("| Status"))
        .skip(2)
        .take_while(|line| line.starts_with('|'))
        .collect();
    let codes: Vec<&str> = table
        .iter()
        .filter_map(|row| row.split('|').nth(1))
        .map(str::trim)
        .filter(|cell| !cell.is_empty() && cell.chars().all(|c| c.is_ascii_digit()))
        .collect();
    let want: Vec<&str> = EXIT_STATUS.iter().map(|(code, _)| *code).collect();
    assert_eq!(codes, want, "the exit table in docs/scripting.md");
    assert!(table.join("\n").contains("a failed `doctor` check"));
    assert!(root_help().contains("a failed doctor check"));
}

fn root_help() -> String {
    help::render(None, 100, PLAIN)
}

fn page(topic: &str, width: usize) -> String {
    help::render(spec::command(topic), width, PLAIN)
}

fn versionless(text: String) -> String {
    text.replace(
        concat!("sshpick ", env!("CARGO_PKG_VERSION")),
        "sshpick [version]",
    )
}

#[test]
fn the_root_help() {
    assert_snapshot!("help_root", versionless(help::render(None, 80, PLAIN)));
}

#[test_case("pick")]
#[test_case("list")]
#[test_case("inspect")]
#[test_case("doctor")]
#[test_case("help")]
fn each_commands_help(topic: &str) {
    assert_snapshot!(format!("help_{topic}"), page(topic, 80));
}

#[test_case(40)]
#[test_case(100)]
fn the_root_help_wraps_to_the_width(width: usize) {
    assert_snapshot!(
        format!("help_root_{width}"),
        versionless(help::render(None, width, PLAIN))
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
        .filter(|option| option.hidden)
        .map(|option| option.long)
        .collect();
    assert_eq!(hidden, ["list"]);
    let topics = std::iter::once(None).chain(COMMANDS.iter().copied().map(Some));
    for topic in topics {
        let text = help::render(topic, 80, PLAIN);
        assert!(!text.contains("--list"), "{text}");
        assert_eq!(
            text.contains("--json"),
            topic.is_none_or(|spec| !spec.options.is_empty()),
            "the root page names --json once, in the line for scripts and agents"
        );
        assert!(!text.contains(" -v"), "{text}");
    }
}

#[test]
fn every_option_in_the_table_parses() {
    let spellings = |option: &spec::OptionSpec| {
        let mut words = vec![format!("--{}", option.long)];
        words.extend(option.short.map(|short| format!("-{short}")));
        words.extend(option.aliases.iter().map(|alias| (*alias).to_owned()));
        words
    };
    let mut cases: Vec<Vec<String>> = GLOBAL
        .iter()
        .flat_map(spellings)
        .map(|word| vec![word])
        .collect();
    for spec in COMMANDS {
        for option in spec.options {
            for word in spellings(option) {
                cases.push(vec![spec.name.to_owned(), word]);
            }
        }
    }
    for args in cases {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let parsed = command(&args);
        assert!(
            !matches!(parsed, Err(UsageError::Option { .. })),
            "{args:?} is in spec.rs but does not parse: {parsed:?}"
        );
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

fn several(count: usize, refusal: Refusal) -> pick::Error {
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
            Error::Pick(several(4, Refusal::NoTerminal)),
        ),
        (
            "TERM=dumb sshpick consul",
            Error::Pick(several(4, Refusal::DumbTerminal)),
        ),
        (
            "sshpick --non-interactive",
            Error::Pick(pick::Error::Several {
                count: 12,
                query: String::new(),
                refusal: Refusal::NonInteractive,
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
            "sshpick doctor extra",
            Error::Usage(UsageError::Unexpected {
                command: "doctor",
                word: "extra".to_owned(),
            }),
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

#[test]
fn the_picker_page_lists_the_keys_the_picker_acts_on() {
    let text = page("pick", 100);
    for binding in sshpick_core::BINDINGS {
        let listed = text
            .lines()
            .any(|line| line.contains(binding.keys) && line.contains(binding.does));
        assert!(listed, "pick --help lacks: {binding:?}");
    }
    assert!(text.contains("?, F1"));
    assert!(!page("list", 100).contains("Keys:"));
}
