use std::ffi::OsString;
use std::io;

use insta::assert_snapshot;
use test_case::test_case;

use super::args::{Command, UsageError, parse};
use super::error::{Error, Exit};
use super::help::HELP;
use crate::commands::{list, pick};

fn argv(args: &[&str]) -> Result<Command, UsageError> {
    parse(args.iter().map(OsString::from))
}

fn pick_for(query: &str) -> Result<Command, UsageError> {
    Ok(Command::Pick(query.to_owned()))
}

fn list_for(query: &str) -> Result<Command, UsageError> {
    Ok(Command::List(query.to_owned()))
}

fn unknown(option: &str) -> Result<Command, UsageError> {
    Err(UsageError(option.to_owned()))
}

#[test_case(&[] => pick_for(""); "no arguments open the picker")]
#[test_case(&["new", "consul"] => pick_for("new consul"); "words join into one query")]
#[test_case(&["-h"] => Ok(Command::Help); "short help")]
#[test_case(&["--version"] => Ok(Command::Version); "long version")]
#[test_case(&["db", "-h"] => Ok(Command::Help); "help after a word")]
#[test_case(&["--", "-h"] => pick_for("-h"); "after -- an option is query text")]
#[test_case(&["--bogus"] => unknown("--bogus"); "an unknown long option")]
#[test_case(&["-l", "db"] => list_for("db"); "short list with a query")]
#[test_case(&["db", "--list"] => list_for("db"); "long list after a word")]
#[test_case(&["-l"] => list_for(""); "list with no query")]
#[test_case(&["--list", "--", "-x", "y"] => list_for("-x y"); "list with a dashed query")]
#[test_case(&["a", "--", "-h"] => pick_for("a -h"); "words before and after --")]
#[test_case(&["--", "--", "-l"] => pick_for("-- -l"); "a second -- is query text")]
#[test_case(&["--"] => pick_for(""); "a bare --")]
#[test_case(&["-"] => pick_for("-"); "a lone dash is query text")]
#[test_case(&["-lh"] => unknown("-lh"); "clustered short options are unknown")]
#[test_case(&["-l", "-x"] => unknown("-x"); "an unknown option after -l")]
#[test_case(&["-h", "--bogus"] => Ok(Command::Help); "help first wins")]
#[test_case(&["--bogus", "-h"] => unknown("--bogus"); "an unknown option first wins")]
fn argv_is_read_left_to_right_up_to_a_double_dash(args: &[&str]) -> Result<Command, UsageError> {
    argv(args)
}

#[test_case(Error::Usage(UsageError("--x".to_owned())), 2; "usage")]
#[test_case(Error::NoHome, 2; "no home")]
#[test_case(Error::NoHosts, 2; "no hosts")]
#[test_case(Error::List(list::Error::Write(io::Error::other("x"))), 2; "list write")]
#[test_case(Error::Pick(pick::Error::NoTerminal), 2; "no terminal")]
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
    for code in ["1 ", "2 ", "127 ", "130 "] {
        assert!(HELP.contains(code), "HELP lacks exit code {code}");
    }
}

#[test]
fn help_fits_a_100_column_terminal() {
    for line in HELP.lines() {
        assert!(line.chars().count() < 100, "{line}");
    }
}

#[test]
fn help_leads_with_the_banner_that_version_prints() {
    let banner = format!("sshpick {}\n", env!("CARGO_PKG_VERSION"));
    assert!(HELP.starts_with(&banner), "{HELP}");
}

#[test]
fn help_reads_as_it_always_has() {
    assert_snapshot!("help", HELP);
}

#[test]
fn a_usage_error_names_the_option_and_points_at_help() {
    assert_snapshot!("usage_error", UsageError("--bogus".to_owned()).to_string());
}
