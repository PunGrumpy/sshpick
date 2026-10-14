mod ui;

use std::ffi::OsString;
use std::fmt::Display;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use sshpick_core::{Entry, Host, Key, Loaded, Outcome, Picker, Previews, Warning};

use crate::ui::Screen;

const BUSY_POLL: Duration = Duration::from_millis(15);
const IDLE_POLL: Duration = Duration::from_secs(3600);

const HELP: &str = concat!(
    "sshpick ",
    env!("CARGO_PKG_VERSION"),
    "\n",
    "\
Pick a host from ~/.ssh/config (and its Includes) and ssh to it.

Usage:
  sshpick [query...]
  sshpick --list [query...]

Arguments:
  query            Seeds the filter; words are joined with spaces.
                   fzf syntax: 'exact ^prefix suffix$ !not
                   Exactly one match connects at once, without opening the picker.

Options:
  -l, --list       Print the matching hosts as ENV, HOST, WHAT (tab-separated) and exit
  -h, --help       Show this help
  -V, --version    Show the version
  --               Everything after it is query text, even if it starts with -

Keys:
  type             filter               ↑↓ or Ctrl-P/Ctrl-N   move
  Enter            ssh                  Esc or Ctrl-C         quit
  Ctrl-U           clear the query      Ctrl-W                delete the last word

Environment:
  SSHPICK_THEME    dark or light; overrides the COLORFGBG detection
  NO_COLOR         draw with reverse video, bold and dim only (CLICOLOR_FORCE re-enables)

Exit status:
  0 success (after a pick, ssh's own status)  1 no match
  2 config, usage or terminal error           127 ssh not runnable    130 cancelled

Examples:
  sshpick                  Open the picker with every host
  sshpick new consul       Open it filtered to hosts matching both words
  sshpick --list | fzf     Pipe the host table to another tool
"
);

const NO_TERMINAL: &str = "needs an interactive terminal; use --list to print the hosts";
const NO_HOSTS: &str = "no hosts found in ~/.ssh/config or the files it Includes\n\
Hosts come from 'Host' lines; see README for the ENV/WHAT comment conventions.";

#[derive(Clone, Copy, Debug)]
enum Exit {
    NoMatch = 1,
    Error = 2,
    ExecFailed = 127,
    Cancelled = 130,
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Version,
    Usage(String),
    List(String),
    Pick(String),
}

fn main() -> ExitCode {
    match Command::parse(std::env::args_os().skip(1)) {
        Command::Help => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("sshpick {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Usage(option) => fail(
            Exit::Error,
            format_args!("unknown option '{option}'\nRun 'sshpick --help' for usage."),
        ),
        Command::List(query) => with_hosts(|loaded| list(loaded, &query)),
        Command::Pick(query) => with_hosts(|loaded| pick(loaded, &query)),
    }
}

fn with_hosts(run: impl FnOnce(Loaded) -> ExitCode) -> ExitCode {
    let Some(home) = std::env::home_dir() else {
        return fail(Exit::Error, "cannot find the home directory");
    };
    let loaded = match sshpick_core::load(&home) {
        Ok(loaded) => loaded,
        Err(error) => return fail(Exit::Error, error),
    };
    if loaded.entries.is_empty() {
        report(&loaded.warnings);
        return fail(Exit::Error, NO_HOSTS);
    }
    run(loaded)
}

fn list(loaded: Loaded, query: &str) -> ExitCode {
    report(&loaded.warnings);
    let picker = Picker::new(loaded.entries, query);
    let rows = picker.listing();
    if rows.is_empty() {
        return Exit::NoMatch.into();
    }
    match write_rows(&rows) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => fail(
            Exit::Error,
            format_args!("cannot write the host list: {error}"),
        ),
    }
}

fn write_rows(rows: &[&Entry]) -> io::Result<()> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    for entry in rows {
        let (env, what) = (field(&entry.env), field(&entry.what));
        writeln!(out, "{env}\t{}\t{what}", entry.host)?;
    }
    out.flush()
}

/// A tab inside a comment would add a column and break `cut -f2`.
fn field(text: &str) -> String {
    text.replace('\t', " ")
}

fn pick(loaded: Loaded, query: &str) -> ExitCode {
    let screen = Screen::new(&loaded.entries, &loaded.warnings);
    let mut picker = Picker::new(loaded.entries, query);
    if let Some(host) = picker.only_match() {
        report(&loaded.warnings);
        return exec(host);
    }
    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        report(&loaded.warnings);
        return fail(Exit::Error, NO_TERMINAL);
    }
    let mut previews = match Previews::spawn() {
        Ok(previews) => previews,
        Err(error) => {
            report(&loaded.warnings);
            return fail(Exit::Error, format_args!("cannot start previews: {error}"));
        }
    };
    if let Some(host) = picker.selected() {
        previews.prefetch(host);
    }

    match run(&screen, &mut picker, &mut previews) {
        Ok(Outcome::Pick(host)) => exec(&host),
        Ok(Outcome::Cancel) => Exit::Cancelled.into(),
        Ok(Outcome::NoMatch) => Exit::NoMatch.into(),
        Err(error) => {
            report(&loaded.warnings);
            fail(Exit::Error, error)
        }
    }
}

/// Warnings go to stderr only when there is no status bar to show them in.
fn report(warnings: &[Warning]) {
    for warning in warnings {
        eprintln!("sshpick: warning: {warning}");
    }
}

struct Tty {
    terminal: DefaultTerminal,
    active: bool,
}

impl Tty {
    fn enter() -> io::Result<Tty> {
        match ratatui::try_init() {
            Ok(terminal) => Ok(Tty {
                terminal,
                active: true,
            }),
            Err(error) => {
                // A half-done init may have enabled raw mode before failing.
                let _ = ratatui::try_restore();
                Err(error)
            }
        }
    }

    fn leave(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        let cursor = self.terminal.show_cursor();
        let restored = ratatui::try_restore();
        self.active = false;
        cursor.and(restored)
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

fn run(screen: &Screen, picker: &mut Picker, previews: &mut Previews) -> io::Result<Outcome> {
    let mut tty = Tty::enter()?;
    let outcome = events(&mut tty, screen, picker, previews);
    tty.leave()?;
    outcome
}

fn events(
    tty: &mut Tty,
    screen: &Screen,
    picker: &mut Picker,
    previews: &mut Previews,
) -> io::Result<Outcome> {
    loop {
        let selected = picker.selected().cloned();
        let pane = selected.as_ref().map(|host| (host, previews.fetch(host)));
        tty.terminal
            .draw(|frame| screen.draw(frame, picker, pane))?;

        let timeout = if previews.busy() {
            BUSY_POLL
        } else {
            IDLE_POLL
        };
        if !event::poll(timeout)? {
            continue;
        }
        if let Event::Key(event) = event::read()?
            && let Some(outcome) = key(event).and_then(|key| picker.handle(key))
        {
            return Ok(outcome);
        }
    }
}

fn key(event: KeyEvent) -> Option<Key> {
    if event.kind != KeyEventKind::Press {
        return None;
    }
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    let alt = event.modifiers.contains(KeyModifiers::ALT);
    match event.code {
        KeyCode::Char(c) if ctrl => Some(Key::Ctrl(c.to_ascii_lowercase())),
        KeyCode::Char(c) if !alt => Some(Key::Char(c)),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Esc => Some(Key::Esc),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        _ => None,
    }
}

fn exec(host: &Host) -> ExitCode {
    let error = sshpick_core::exec(host);
    fail(Exit::ExecFailed, format_args!("cannot run ssh: {error}"))
}

fn fail(exit: Exit, message: impl Display) -> ExitCode {
    eprintln!("sshpick: {message}");
    exit.into()
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> ExitCode {
        ExitCode::from(exit as u8)
    }
}

impl Command {
    fn parse(args: impl IntoIterator<Item = OsString>) -> Command {
        let mut args = args
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned());
        let mut list = false;
        let mut words = Vec::new();
        for arg in args.by_ref() {
            match arg.as_str() {
                "--" => break,
                "-h" | "--help" => return Command::Help,
                "-V" | "--version" => return Command::Version,
                "-l" | "--list" => list = true,
                _ if arg.starts_with('-') && arg.len() > 1 => return Command::Usage(arg),
                _ => words.push(arg),
            }
        }
        words.extend(args);
        let query = words.join(" ");
        if list {
            Command::List(query)
        } else {
            Command::Pick(query)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_the_documented_ones() {
        assert_eq!(ExitCode::from(Exit::NoMatch), ExitCode::from(1));
        assert_eq!(ExitCode::from(Exit::Error), ExitCode::from(2));
        assert_eq!(ExitCode::from(Exit::ExecFailed), ExitCode::from(127));
        assert_eq!(ExitCode::from(Exit::Cancelled), ExitCode::from(130));
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

    fn parse(args: &[&str]) -> Command {
        Command::parse(args.iter().map(OsString::from))
    }

    #[test]
    fn argv_joins_into_one_query() {
        assert_eq!(parse(&[]), Command::Pick(String::new()));
        assert_eq!(
            parse(&["new", "consul"]),
            Command::Pick("new consul".to_owned())
        );
        assert_eq!(parse(&["-h"]), Command::Help);
        assert_eq!(parse(&["--version"]), Command::Version);
        assert_eq!(parse(&["db", "-h"]), Command::Help);
        assert_eq!(parse(&["--", "-h"]), Command::Pick("-h".to_owned()));
        assert_eq!(parse(&["--bogus"]), Command::Usage("--bogus".to_owned()));
        assert_eq!(parse(&["-l", "db"]), Command::List("db".to_owned()));
    }

    #[test]
    fn options_are_read_left_to_right_up_to_a_double_dash() {
        assert_eq!(parse(&["db", "--list"]), Command::List("db".to_owned()));
        assert_eq!(parse(&["-l"]), Command::List(String::new()));
        assert_eq!(
            parse(&["--list", "--", "-x", "y"]),
            Command::List("-x y".to_owned())
        );
        assert_eq!(parse(&["a", "--", "-h"]), Command::Pick("a -h".to_owned()));
        assert_eq!(
            parse(&["--", "--", "-l"]),
            Command::Pick("-- -l".to_owned())
        );
        assert_eq!(parse(&["--"]), Command::Pick(String::new()));
        assert_eq!(
            parse(&["-"]),
            Command::Pick("-".to_owned()),
            "a lone dash is query text"
        );
        assert_eq!(parse(&["-lh"]), Command::Usage("-lh".to_owned()));
        assert_eq!(parse(&["-l", "-x"]), Command::Usage("-x".to_owned()));
        assert_eq!(parse(&["-h", "--bogus"]), Command::Help, "first one wins");
        assert_eq!(
            parse(&["--bogus", "-h"]),
            Command::Usage("--bogus".to_owned())
        );
    }

    #[test]
    fn key_maps_without_policy() {
        let press = |code, modifiers| key(KeyEvent::new(code, modifiers));
        let ctrl = KeyModifiers::CONTROL;
        assert_eq!(press(KeyCode::Char('p'), ctrl), Some(Key::Ctrl('p')));
        assert_eq!(press(KeyCode::Char('J'), ctrl), Some(Key::Ctrl('j')));
        assert_eq!(
            press(KeyCode::Char('A'), KeyModifiers::SHIFT),
            Some(Key::Char('A'))
        );
        assert_eq!(press(KeyCode::Char('x'), KeyModifiers::ALT), None);
        assert_eq!(press(KeyCode::Left, KeyModifiers::NONE), None);
    }
}
