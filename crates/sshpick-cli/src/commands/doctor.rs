use std::io::{self, Write};
use std::time::Instant;

use serde::Serialize;
use sshpick_core::{Check, Config, Facts, Probe, Report, Status, Terminal, printable};
use sshpick_ui::{ColorConfig, Output};

use super::{interaction, note_if_also_a_host, write_json};
use crate::cli;
use crate::cli::Exit;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("cannot write the report: {0}")]
    Write(#[source] io::Error),
}

pub(crate) struct Request<'a> {
    pub(crate) json: bool,
    pub(crate) non_interactive: bool,
    pub(crate) typed: &'a str,
}

#[derive(Serialize)]
struct View<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<String>,
    summary: Summary,
    checks: Vec<CheckView<'a>>,
}

#[derive(Serialize)]
struct Summary {
    ok: usize,
    warn: usize,
    fail: usize,
}

#[derive(Serialize)]
struct CheckView<'a> {
    name: &'static str,
    status: &'static str,
    detail: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<&'a str>,
}

/// Does not go through `CommandBase`, which fails without a home directory: that is one of the
/// things to report.
pub(crate) fn run(output: &Output, request: &Request) -> Result<u8, cli::Error> {
    let facts = gather(output, request.non_interactive);
    if let Config::Read { loaded, .. } = &facts.config
        && !request.json
    {
        note_if_also_a_host(output, request.typed, &loaded.entries);
    }
    let report = sshpick_core::report(&facts);
    let mut out = output.result();
    if request.json {
        write_json(out, &view(&report)).map_err(Error::Write)?;
    } else {
        out.write_all(text(&report, output.stdout_colors()).as_bytes())
            .and_then(|()| out.flush())
            .map_err(Error::Write)?;
    }
    let code = if report.failed() {
        Exit::Error
    } else {
        Exit::Success
    };
    Ok(code.code())
}

fn gather(output: &Output, non_interactive: bool) -> Facts {
    let config = match std::env::home_dir() {
        None => Config::NoHome,
        Some(home) => {
            let path = sshpick_core::config_path(&home);
            match sshpick_core::load(&home) {
                Ok(loaded) => Config::Read { path, loaded },
                Err(error) => Config::Failed { path, error },
            }
        }
    };
    let ssh = sshpick_core::ssh_version();
    let probe = probe(&config, &ssh);
    let colors = output.stdout_colors();
    let terminal = Terminal {
        interaction: interaction(non_interactive),
        size: output.size(),
        term: std::env::var_os("TERM").map(|term| term.to_string_lossy().into_owned()),
        color: colors.color(),
        color_reason: colors.reason().words(),
    };
    Facts {
        config,
        ssh,
        probe,
        terminal,
    }
}

/// One `ssh -G` against the first host: one bad option anywhere in the config fails every host.
fn probe(config: &Config, ssh: &io::Result<String>) -> Option<Probe> {
    let (Config::Read { loaded, .. }, Ok(_)) = (config, ssh) else {
        return None;
    };
    let host = &loaded.entries.first()?.host;
    let started = Instant::now();
    let result = sshpick_core::resolve(host).map(drop);
    Some(Probe {
        host: host.clone(),
        result,
        took: started.elapsed(),
    })
}

fn text(report: &Report, colors: ColorConfig) -> String {
    let count = |status| report.count(status);
    let mut text = format!(
        "[doctor] ok={} warn={} fail={}\n",
        count(Status::Ok),
        count(Status::Warn),
        count(Status::Fail)
    );
    if let Some(config) = &report.config {
        let config = printable(&config.display().to_string());
        text.push_str(&format!("[doctor] config={config}\n"));
    }
    for check in &report.checks {
        text.push_str(&line(check, colors));
    }
    text
}

fn line(check: &Check, colors: ColorConfig) -> String {
    let token = format!("[{}]", check.status.as_str());
    let token = match check.status {
        Status::Ok => token,
        Status::Warn => colors.yellow(&token),
        Status::Fail => colors.red(&token),
    };
    let fix = check
        .fix
        .as_ref()
        .map_or_else(String::new, |fix| format!("; {fix}"));
    format!("{token} {}: {}{fix}\n", check.name.as_str(), check.detail)
}

fn view(report: &Report) -> View<'_> {
    View {
        config: report
            .config
            .as_ref()
            .map(|config| printable(&config.display().to_string())),
        summary: Summary {
            ok: report.count(Status::Ok),
            warn: report.count(Status::Warn),
            fail: report.count(Status::Fail),
        },
        checks: report
            .checks
            .iter()
            .map(|check| CheckView {
                name: check.name.as_str(),
                status: check.status.as_str(),
                detail: &check.detail,
                fix: check.fix.as_deref(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    fn check(status: Status, name: sshpick_core::Name, fix: Option<&str>) -> Check {
        Check {
            name,
            status,
            detail: "the detail".to_owned(),
            fix: fix.map(str::to_owned),
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

    #[test_case(Status::Ok, None, "[ok] ssh: the detail\n"; "ok")]
    #[test_case(Status::Warn, Some("do this"), "[warn] ssh: the detail; do this\n"; "warn with a fix")]
    #[test_case(Status::Fail, Some("install `ssh`"), "[fail] ssh: the detail; install `ssh`\n"; "fail with a fix")]
    #[test_case(Status::Fail, None, "[fail] ssh: the detail\n"; "fail with none")]
    fn a_check_is_one_line_with_its_fix_after_a_semicolon(
        status: Status,
        fix: Option<&str>,
        want: &str,
    ) {
        let check = check(status, sshpick_core::Name::Ssh, fix);
        assert_eq!(line(&check, ColorConfig::none()), want);
    }

    #[test]
    fn only_the_bracketed_token_is_coloured_and_ok_is_not() {
        let colors = ColorConfig::new(true, false, &|_| None);
        let name = sshpick_core::Name::Hosts;
        assert_eq!(
            line(&check(Status::Ok, name, None), colors),
            "[ok] hosts: the detail\n"
        );
        assert_eq!(
            line(&check(Status::Warn, name, Some("fix")), colors),
            "\x1b[33m[warn]\x1b[39m hosts: the detail; fix\n"
        );
        assert_eq!(
            line(&check(Status::Fail, name, None), colors),
            "\x1b[31m[fail]\x1b[39m hosts: the detail\n"
        );
        let no_hue = ColorConfig::new(true, true, &|_| None);
        assert_eq!(
            line(&check(Status::Fail, name, None), no_hue),
            "[fail] hosts: the detail\n"
        );
    }

    fn report() -> Report {
        Report {
            config: Some("/h/.ssh/config".into()),
            checks: vec![
                check(Status::Ok, sshpick_core::Name::Config, None),
                check(Status::Warn, sshpick_core::Name::Terminal, Some("fix")),
                check(Status::Fail, sshpick_core::Name::Ssh, Some("install")),
            ],
        }
    }

    #[test]
    fn the_report_starts_with_the_counts_and_the_config_it_read() {
        let text = text(&report(), ColorConfig::none());
        assert_eq!(
            text,
            "[doctor] ok=1 warn=1 fail=1\n\
             [doctor] config=/h/.ssh/config\n\
             [ok] config: the detail\n\
             [warn] terminal: the detail; fix\n\
             [fail] ssh: the detail; install\n"
        );
        let colors = ColorConfig::new(true, false, &|_| None);
        assert_eq!(strip_escapes(&super::text(&report(), colors)), text);
    }

    #[test]
    fn without_a_config_path_there_is_no_config_line() {
        let mut report = report();
        report.config = None;
        let text = text(&report, ColorConfig::none());
        assert_eq!(text.lines().count(), 4);
        assert!(!text.contains("config="));
    }

    #[test]
    fn json_has_the_summary_and_leaves_out_a_missing_config_and_a_missing_fix() {
        let mut report = report();
        let mut out = Vec::new();
        write_json(&mut out, &view(&report)).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("{\n  \"config\": \"/h/.ssh/config\",\n  \"summary\": {\n"));
        assert!(text.ends_with("}\n"));
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value["summary"],
            serde_json::json!({"ok": 1, "warn": 1, "fail": 1})
        );
        assert_eq!(
            value["checks"][0],
            serde_json::json!({"name": "config", "status": "ok", "detail": "the detail"}),
            "no fix key without a fix"
        );
        assert_eq!(value["checks"][2]["fix"], "install");

        report.config = None;
        let value = serde_json::to_value(view(&report)).unwrap();
        assert!(value.get("config").is_none());
    }

    #[test]
    fn the_documented_names_are_the_ones_a_report_can_have() {
        let docs = include_str!("../../../../docs/scripting.md");
        let table: Vec<&str> = docs
            .lines()
            .skip_while(|line| !line.starts_with("### `doctor --json`"))
            .skip_while(|line| !line.starts_with("| Check"))
            .skip(2)
            .take_while(|line| line.starts_with('|'))
            .filter_map(|line| line.split('`').nth(1))
            .collect();
        let names: Vec<&str> = sshpick_core::Name::ALL
            .iter()
            .map(|name| name.as_str())
            .collect();
        assert_eq!(table, names, "the check table in docs/scripting.md");
    }

    #[test]
    fn the_json_example_in_the_docs_has_the_shape_the_command_prints() {
        let docs = include_str!("../../../../docs/scripting.md");
        let example: String = docs
            .lines()
            .skip_while(|line| !line.starts_with("    $ sshpick doctor --json"))
            .skip(1)
            .take_while(|line| line.starts_with("    "))
            .map(|line| format!("{}\n", line.trim_start_matches("    ")))
            .collect();
        let value: serde_json::Value = serde_json::from_str(&example).unwrap();
        let checks = value["checks"].as_array().unwrap();
        for status in ["ok", "warn", "fail"] {
            let counted = checks.iter().filter(|c| c["status"] == status).count();
            assert_eq!(value["summary"][status], counted, "{status}");
        }
        for check in checks {
            assert!(
                sshpick_core::Name::ALL
                    .iter()
                    .any(|name| check["name"] == name.as_str()),
                "{check}"
            );
        }
    }
}
