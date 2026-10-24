use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::{Host, LoadError, Loaded, Warning, printable};
use crate::interaction::{Interaction, Refusal, refusal};
use crate::ssh::ResolveError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Fail => "fail",
        }
    }
}

/// The order a report lists checks in; `Include` appears once per config warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Name {
    Config,
    Hosts,
    Include,
    Ssh,
    Resolve,
    Terminal,
}

impl Name {
    pub const ALL: [Name; 6] = [
        Name::Config,
        Name::Hosts,
        Name::Include,
        Name::Ssh,
        Name::Resolve,
        Name::Terminal,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Name::Config => "config",
            Name::Hosts => "hosts",
            Name::Include => "include",
            Name::Ssh => "ssh",
            Name::Resolve => "resolve",
            Name::Terminal => "terminal",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub name: Name,
    pub status: Status,
    pub detail: String,
    /// The next step, when there is a concrete one.
    pub fix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// The root config file, when there is a home directory to look in.
    pub config: Option<PathBuf>,
    pub checks: Vec<Check>,
}

impl Report {
    pub fn count(&self, status: Status) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == status)
            .count()
    }

    pub fn failed(&self) -> bool {
        self.count(Status::Fail) > 0
    }
}

#[derive(Debug)]
pub enum Config {
    NoHome,
    Read { path: PathBuf, loaded: Loaded },
    Failed { path: PathBuf, error: LoadError },
}

#[derive(Debug)]
pub struct Probe {
    pub host: Host,
    pub result: Result<(), ResolveError>,
    pub took: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terminal {
    pub interaction: Interaction,
    /// Columns and rows, when stdout is a terminal that reports them.
    pub size: Option<(u16, u16)>,
    pub term: Option<String>,
    pub color: bool,
    pub color_reason: &'static str,
}

#[derive(Debug)]
pub struct Facts {
    pub config: Config,
    pub ssh: io::Result<String>,
    /// `None` when there is no host to ask or `ssh -V` failed.
    pub probe: Option<Probe>,
    pub terminal: Terminal,
}

pub fn report(facts: &Facts) -> Report {
    let mut checks = Vec::new();
    let config = match &facts.config {
        Config::NoHome => {
            checks.push(no_home());
            None
        }
        Config::Failed { path, error } => {
            checks.push(unreadable(error));
            Some(path.clone())
        }
        Config::Read { path, loaded } => {
            checks.push(read(path, loaded));
            checks.push(hosts(loaded));
            checks.extend(loaded.warnings.iter().map(include));
            Some(path.clone())
        }
    };
    checks.push(ssh(&facts.ssh));
    checks.extend(facts.probe.as_ref().map(resolve));
    checks.push(terminal(&facts.terminal));
    Report { config, checks }
}

fn check(name: Name, status: Status, detail: String, fix: Option<String>) -> Check {
    Check {
        name,
        status,
        detail: printable(&detail),
        fix: fix.map(|fix| printable(&fix)),
    }
}

fn no_home() -> Check {
    check(
        Name::Config,
        Status::Fail,
        "cannot find the home directory".to_owned(),
        Some("set `HOME`".to_owned()),
    )
}

fn unreadable(error: &LoadError) -> Check {
    check(
        Name::Config,
        Status::Fail,
        error.to_string(),
        Some("create `~/.ssh/config`, or make it readable".to_owned()),
    )
}

fn read(path: &Path, loaded: &Loaded) -> Check {
    let included = match loaded.included {
        0 => String::new(),
        1 => " and 1 included file".to_owned(),
        count => format!(" and {count} included files"),
    };
    check(
        Name::Config,
        Status::Ok,
        format!("read {}{included}", path.display()),
        None,
    )
}

fn hosts(loaded: &Loaded) -> Check {
    let total = loaded.entries.len();
    if total == 0 {
        return check(
            Name::Hosts,
            Status::Fail,
            "no hosts found in the config or the files it Includes".to_owned(),
            Some("add `Host` lines; ENV and WHAT come from comments".to_owned()),
        );
    }
    let noun = if total == 1 { "host" } else { "hosts" };
    let labelled = loaded.entries.iter().filter(|e| !e.env.is_empty()).count();
    let described = loaded.entries.iter().filter(|e| !e.what.is_empty()).count();
    check(
        Name::Hosts,
        Status::Ok,
        format!("{total} {noun}, {labelled} with an ENV label, {described} with a WHAT comment"),
        None,
    )
}

fn include(warning: &Warning) -> Check {
    let fix = match warning {
        Warning::Unreadable { .. } => "make the file readable, or remove the Include that names it",
        Warning::TooDeep { .. } => "flatten the Include chain",
        Warning::Cycle { .. } => "remove the Include that points back",
        Warning::BadInclude { .. } => "fix the Include pattern",
    };
    check(
        Name::Include,
        Status::Warn,
        warning.to_string(),
        Some(fix.to_owned()),
    )
}

fn ssh(version: &io::Result<String>) -> Check {
    match version {
        Ok(line) => check(Name::Ssh, Status::Ok, line.clone(), None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => check(
            Name::Ssh,
            Status::Fail,
            "not found on PATH".to_owned(),
            Some("install OpenSSH, or put `ssh` on your PATH".to_owned()),
        ),
        Err(error) => check(
            Name::Ssh,
            Status::Fail,
            format!("ssh -V failed: {error}"),
            None,
        ),
    }
}

fn resolve(probe: &Probe) -> Check {
    let Probe { host, result, took } = probe;
    match result {
        Ok(()) => check(
            Name::Resolve,
            Status::Ok,
            format!("ssh -G answered for {host} in {} ms", took.as_millis()),
            None,
        ),
        Err(error) => {
            let headline = error.headline();
            let said = headline
                .strip_prefix("ssh -G failed: ")
                .unwrap_or(&headline);
            check(
                Name::Resolve,
                Status::Fail,
                format!("ssh -G failed for {host}: {said}"),
                Some(format!(
                    "run `sshpick inspect {host}` for ssh's full answer"
                )),
            )
        }
    }
}

fn terminal(terminal: &Terminal) -> Check {
    let Terminal {
        interaction,
        size,
        term,
        color,
        color_reason,
    } = terminal;
    let mut seen = Vec::new();
    match size {
        Some((cols, rows)) => seen.push(format!("{cols}x{rows}")),
        None if interaction.stdout_is_terminal => {
            seen.push("size unknown, taken as 80x24".to_owned());
        }
        None => {}
    }
    seen.push(match term {
        Some(term) => format!("TERM={term}"),
        None => "TERM is not set".to_owned(),
    });
    let colour = if *color { "on" } else { "off" };
    seen.push(format!("colour {colour} ({color_reason})"));
    let seen = seen.join(", ");
    let Some(refusal) = refusal(interaction) else {
        return check(
            Name::Terminal,
            Status::Ok,
            format!("{seen}; the picker can open here"),
            None,
        );
    };
    let fix = match refusal {
        Refusal::NonInteractive => {
            "leave out `--non-interactive` to open the picker; `sshpick <query>` still connects when one host matches"
        }
        Refusal::NoTerminal => "`sshpick <query>` still connects when one host matches",
        Refusal::DumbTerminal => {
            "set `TERM` to your terminal's type, such as `xterm-256color`; `sshpick <query>` still connects when one host matches"
        }
    };
    check(
        Name::Terminal,
        Status::Warn,
        format!("{seen}; {}", refusal.why()),
        Some(fix.to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;
    use crate::config::Entry;

    fn entry(host: &str, env: &str, what: &str) -> Entry {
        Entry {
            env: env.to_owned(),
            host: Host::literal(host).unwrap(),
            what: what.to_owned(),
            aliases: Vec::new(),
        }
    }

    fn loaded(entries: Vec<Entry>, warnings: Vec<Warning>, included: usize) -> Loaded {
        Loaded {
            entries,
            warnings,
            included,
        }
    }

    fn read(entries: Vec<Entry>, warnings: Vec<Warning>, included: usize) -> Config {
        Config::Read {
            path: PathBuf::from("/h/.ssh/config"),
            loaded: loaded(entries, warnings, included),
        }
    }

    fn at_a_terminal() -> Terminal {
        Terminal {
            interaction: Interaction {
                non_interactive: false,
                stdin_is_terminal: true,
                stdout_is_terminal: true,
                dumb: false,
            },
            size: Some((80, 24)),
            term: Some("xterm-256color".to_owned()),
            color: true,
            color_reason: "stdout is a terminal",
        }
    }

    fn facts() -> Facts {
        Facts {
            config: read(vec![entry("lab-a", "LAB", "build box")], Vec::new(), 0),
            ssh: Ok("OpenSSH_9.6p1".to_owned()),
            probe: Some(Probe {
                host: Host::literal("lab-a").unwrap(),
                result: Ok(()),
                took: Duration::from_millis(14),
            }),
            terminal: at_a_terminal(),
        }
    }

    fn only(checks: Vec<Check>) -> Check {
        assert_eq!(checks.len(), 1, "{checks:?}");
        checks.into_iter().next().unwrap()
    }

    fn judged(facts: Facts, name: Name) -> Vec<Check> {
        report(&facts)
            .checks
            .into_iter()
            .filter(|check| check.name == name)
            .collect()
    }

    fn shown(check: &Check) -> (Status, &str, Option<&str>) {
        (check.status, check.detail.as_str(), check.fix.as_deref())
    }

    #[test]
    fn a_sound_setup_reports_each_check_once_in_order() {
        let report = report(&facts());
        let names: Vec<_> = report.checks.iter().map(|check| check.name).collect();
        assert_eq!(
            names,
            [
                Name::Config,
                Name::Hosts,
                Name::Ssh,
                Name::Resolve,
                Name::Terminal
            ]
        );
        assert_eq!(report.config.as_deref(), Some(Path::new("/h/.ssh/config")));
        assert_eq!(report.count(Status::Ok), 5);
        assert!(!report.failed());
    }

    #[test]
    fn the_names_are_a_closed_set_in_report_order() {
        let words: Vec<_> = Name::ALL.iter().map(|name| name.as_str()).collect();
        assert_eq!(
            words,
            ["config", "hosts", "include", "ssh", "resolve", "terminal"]
        );
    }

    #[test]
    fn a_warning_is_not_a_failure_and_a_failure_is() {
        let mut warned = facts();
        warned.config = read(
            vec![entry("lab-a", "", "")],
            vec![Warning::Cycle {
                path: PathBuf::from("/h/.ssh/a.conf"),
            }],
            1,
        );
        let report_of_warned = report(&warned);
        assert_eq!(report_of_warned.count(Status::Warn), 1);
        assert!(!report_of_warned.failed());

        let mut failed = facts();
        failed.ssh = Err(io::Error::from(io::ErrorKind::NotFound));
        assert!(report(&failed).failed());
    }

    #[test]
    fn without_a_home_only_the_checks_that_need_none_are_left() {
        let mut no_home = facts();
        no_home.config = Config::NoHome;
        no_home.probe = None;
        let report = report(&no_home);
        let names: Vec<_> = report.checks.iter().map(|check| check.name).collect();
        assert_eq!(names, [Name::Config, Name::Ssh, Name::Terminal]);
        assert_eq!(report.config, None);
        assert_eq!(
            shown(&report.checks[0]),
            (
                Status::Fail,
                "cannot find the home directory",
                Some("set `HOME`")
            )
        );
    }

    #[test]
    fn an_unreadable_root_file_leaves_out_hosts_and_include() {
        let error = crate::config::load(Path::new("/nonexistent-home")).unwrap_err();
        let mut broken = facts();
        broken.config = Config::Failed {
            path: PathBuf::from("/nonexistent-home/.ssh/config"),
            error,
        };
        let report = report(&broken);
        let names: Vec<_> = report.checks.iter().map(|check| check.name).collect();
        assert_eq!(
            names,
            [Name::Config, Name::Ssh, Name::Resolve, Name::Terminal]
        );
        assert_eq!(
            report.config.as_deref(),
            Some(Path::new("/nonexistent-home/.ssh/config"))
        );
        assert_eq!(
            shown(&report.checks[0]),
            (
                Status::Fail,
                "cannot read /nonexistent-home/.ssh/config: No such file or directory (os error 2)",
                Some("create `~/.ssh/config`, or make it readable")
            )
        );
    }

    #[test_case(0, "read /h/.ssh/config"; "no Include")]
    #[test_case(1, "read /h/.ssh/config and 1 included file"; "one file")]
    #[test_case(2, "read /h/.ssh/config and 2 included files"; "two files")]
    fn config_says_what_was_read(included: usize, want: &str) {
        let mut facts = facts();
        facts.config = read(vec![entry("a", "", "")], Vec::new(), included);
        let check = only(judged(facts, Name::Config));
        assert_eq!(shown(&check), (Status::Ok, want, None));
    }

    type Row<'a> = (&'a str, &'a str);

    #[test_case(&[], Status::Fail, "no hosts found in the config or the files it Includes", Some("add `Host` lines; ENV and WHAT come from comments"); "none")]
    #[test_case(&[("", "")], Status::Ok, "1 host, 0 with an ENV label, 0 with a WHAT comment", None; "one bare host")]
    #[test_case(&[("LAB", "build box"), ("LAB", ""), ("", "bastion")], Status::Ok, "3 hosts, 2 with an ENV label, 2 with a WHAT comment", None; "mixed")]
    fn hosts_counts_the_labels_and_comments(
        rows: &[Row],
        status: Status,
        detail: &str,
        fix: Option<&str>,
    ) {
        let entries = rows
            .iter()
            .enumerate()
            .map(|(at, (env, what))| entry(&format!("h{at}"), env, what))
            .collect();
        let mut facts = facts();
        facts.config = read(entries, Vec::new(), 0);
        assert_eq!(
            shown(&only(judged(facts, Name::Hosts))),
            (status, detail, fix)
        );
    }

    fn cycle() -> Warning {
        Warning::Cycle {
            path: PathBuf::from("/h/.ssh/9-late.conf"),
        }
    }

    #[test_case(cycle(), "/h/.ssh/9-late.conf: skipped, include cycle", "remove the Include that points back"; "cycle")]
    #[test_case(Warning::TooDeep { path: PathBuf::from("/h/.ssh/d17.conf") }, "/h/.ssh/d17.conf: skipped, Include nested deeper than 16 levels", "flatten the Include chain"; "too deep")]
    #[test_case(Warning::Unreadable { path: PathBuf::from("/h/.ssh/b.conf"), source: io::Error::from(io::ErrorKind::PermissionDenied) }, "/h/.ssh/b.conf: skipped, permission denied", "make the file readable, or remove the Include that names it"; "unreadable")]
    #[test_case(Warning::BadInclude { arg: "%h.conf".to_owned(), reason: "%h is not supported".to_owned() }, "Include %h.conf: skipped, %h is not supported", "fix the Include pattern"; "bad include")]
    fn each_config_warning_is_one_warn_with_its_own_fix(warning: Warning, detail: &str, fix: &str) {
        let said = warning.to_string();
        let mut facts = facts();
        facts.config = read(vec![entry("a", "", "")], vec![warning], 1);
        assert_eq!(
            said, detail,
            "the detail is the line pick prints after `! `"
        );
        assert_eq!(
            shown(&only(judged(facts, Name::Include))),
            (Status::Warn, detail, Some(fix))
        );
    }

    #[test]
    fn every_warning_gets_its_own_check_after_hosts() {
        let mut facts = facts();
        facts.config = read(vec![entry("a", "", "")], vec![cycle(), cycle()], 2);
        let names: Vec<_> = report(&facts).checks.iter().map(|c| c.name).collect();
        assert_eq!(
            names,
            [
                Name::Config,
                Name::Hosts,
                Name::Include,
                Name::Include,
                Name::Ssh,
                Name::Resolve,
                Name::Terminal
            ]
        );
    }

    #[test_case(Ok("OpenSSH_9.6p1, OpenSSL 3.0.13".to_owned()), Status::Ok, "OpenSSH_9.6p1, OpenSSL 3.0.13", None; "a version")]
    #[test_case(Err(io::Error::from(io::ErrorKind::NotFound)), Status::Fail, "not found on PATH", Some("install OpenSSH, or put `ssh` on your PATH"); "not on PATH")]
    #[test_case(Err(io::Error::from(io::ErrorKind::PermissionDenied)), Status::Fail, "ssh -V failed: permission denied", None; "not executable")]
    #[test_case(Err(io::Error::new(io::ErrorKind::TimedOut, "no answer within 2 s")), Status::Fail, "ssh -V failed: no answer within 2 s", None; "hung")]
    #[test_case(Err(io::Error::other("exited with 255")), Status::Fail, "ssh -V failed: exited with 255", None; "failed")]
    fn ssh_shows_the_version_or_why_there_is_none(
        version: io::Result<String>,
        status: Status,
        detail: &str,
        fix: Option<&str>,
    ) {
        let mut facts = facts();
        facts.ssh = version;
        assert_eq!(
            shown(&only(judged(facts, Name::Ssh))),
            (status, detail, fix)
        );
    }

    fn ssh_said(stderr: &str) -> ResolveError {
        ResolveError::Ssh {
            code: Some(255),
            stderr: stderr.to_owned(),
        }
    }

    #[test_case(Ok(()), Status::Ok, "ssh -G answered for lab-a in 14 ms", None; "answered")]
    #[test_case(Err(ssh_said("/h/.ssh/config line 12: Bad configuration option: foo\n/h/.ssh/config: terminating")), Status::Fail, "ssh -G failed for lab-a: /h/.ssh/config line 12: Bad configuration option: foo", Some("run `sshpick inspect lab-a` for ssh's full answer"); "ssh rejects the config")]
    #[test_case(Err(ssh_said("")), Status::Fail, "ssh -G failed for lab-a: ssh -G exited with 255", Some("run `sshpick inspect lab-a` for ssh's full answer"); "ssh says nothing")]
    #[test_case(Err(ResolveError::Timeout), Status::Fail, "ssh -G failed for lab-a: ssh -G gave no answer within 2 s", Some("run `sshpick inspect lab-a` for ssh's full answer"); "no answer")]
    fn resolve_times_ssh_g_for_the_first_host(
        result: Result<(), ResolveError>,
        status: Status,
        detail: &str,
        fix: Option<&str>,
    ) {
        let mut facts = facts();
        facts.probe = Some(Probe {
            host: Host::literal("lab-a").unwrap(),
            result,
            took: Duration::from_millis(14),
        });
        assert_eq!(
            shown(&only(judged(facts, Name::Resolve))),
            (status, detail, fix)
        );
    }

    #[test]
    fn resolve_is_left_out_when_there_is_nothing_to_ask() {
        let mut facts = facts();
        facts.probe = None;
        assert!(judged(facts, Name::Resolve).is_empty());
    }

    const PICKER_OPENS: &str = "; the picker can open here";
    const FIX_TERMINAL: &str = "`sshpick <query>` still connects when one host matches";

    //        flag   stdin  stdout dumb   size             term                  colour reason
    #[test_case(false, true, true, false, Some((80, 24)), Some("xterm-256color"), false, "NO_COLOR is set",
        Status::Ok, "80x24, TERM=xterm-256color, colour off (NO_COLOR is set); the picker can open here", None;
        "a terminal with NO_COLOR")]
    #[test_case(false, true, true, false, Some((120, 30)), Some("xterm-256color"), true, "stdout is a terminal",
        Status::Ok, "120x30, TERM=xterm-256color, colour on (stdout is a terminal); the picker can open here", None;
        "a terminal with colour")]
    #[test_case(false, true, true, false, None, None, true, "stdout is a terminal",
        Status::Ok, "size unknown, taken as 80x24, TERM is not set, colour on (stdout is a terminal); the picker can open here", None;
        "a terminal that reports no size and no TERM")]
    #[test_case(false, true, false, false, None, Some("xterm"), false, "stdout is not a terminal",
        Status::Warn, "TERM=xterm, colour off (stdout is not a terminal); the picker needs an interactive terminal", Some(FIX_TERMINAL);
        "stdout piped")]
    #[test_case(false, false, true, false, Some((80, 24)), Some("xterm"), true, "stdout is a terminal",
        Status::Warn, "80x24, TERM=xterm, colour on (stdout is a terminal); the picker needs an interactive terminal", Some(FIX_TERMINAL);
        "stdin piped")]
    #[test_case(false, true, true, true, Some((80, 24)), Some("dumb"), false, "TERM=dumb",
        Status::Warn, "80x24, TERM=dumb, colour off (TERM=dumb); the picker needs a terminal that moves the cursor (TERM=dumb)",
        Some("set `TERM` to your terminal's type, such as `xterm-256color`; `sshpick <query>` still connects when one host matches");
        "TERM=dumb")]
    #[test_case(true, true, true, false, Some((80, 24)), Some("xterm"), true, "stdout is a terminal",
        Status::Warn, "80x24, TERM=xterm, colour on (stdout is a terminal); --non-interactive never opens the picker",
        Some("leave out `--non-interactive` to open the picker; `sshpick <query>` still connects when one host matches");
        "--non-interactive")]
    #[allow(clippy::too_many_arguments)]
    fn terminal_says_what_the_picker_would_find(
        non_interactive: bool,
        stdin_is_terminal: bool,
        stdout_is_terminal: bool,
        dumb: bool,
        size: Option<(u16, u16)>,
        term: Option<&str>,
        color: bool,
        color_reason: &'static str,
        status: Status,
        detail: &str,
        fix: Option<&str>,
    ) {
        let mut facts = facts();
        facts.terminal = Terminal {
            interaction: Interaction {
                non_interactive,
                stdin_is_terminal,
                stdout_is_terminal,
                dumb,
            },
            size,
            term: term.map(str::to_owned),
            color,
            color_reason,
        };
        assert_eq!(
            shown(&only(judged(facts, Name::Terminal))),
            (status, detail, fix)
        );
    }

    #[test]
    fn terminal_warns_exactly_when_the_picker_would_refuse_and_gives_pick_s_reason() {
        for bits in 0..16u8 {
            let interaction = Interaction {
                non_interactive: bits & 1 != 0,
                stdin_is_terminal: bits & 2 != 0,
                stdout_is_terminal: bits & 4 != 0,
                dumb: bits & 8 != 0,
            };
            let mut facts = facts();
            facts.terminal.interaction = interaction;
            let check = only(judged(facts, Name::Terminal));
            match refusal(&interaction) {
                None => {
                    assert_eq!(check.status, Status::Ok, "{interaction:?}");
                    assert!(check.detail.ends_with(PICKER_OPENS), "{interaction:?}");
                    assert_eq!(check.fix, None);
                }
                Some(refusal) => {
                    assert_eq!(check.status, Status::Warn, "{interaction:?}");
                    assert!(check.detail.ends_with(refusal.why()), "{}", check.detail);
                    assert!(check.fix.is_some(), "{interaction:?}");
                }
            }
        }
    }

    #[test]
    fn a_control_character_from_the_config_never_reaches_the_report() {
        let mut facts = facts();
        facts.probe = Some(Probe {
            host: Host::literal("a").unwrap(),
            result: Err(ssh_said("bad\u{7}option")),
            took: Duration::ZERO,
        });
        facts.config = read(
            vec![entry("a", "", "")],
            vec![Warning::Cycle {
                path: PathBuf::from("/h/\u{1b}[31m.conf"),
            }],
            1,
        );
        for check in report(&facts).checks {
            let text = format!("{}{}", check.detail, check.fix.unwrap_or_default());
            assert!(!text.contains(char::is_control), "{text:?}");
        }
    }
}
