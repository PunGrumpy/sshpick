use std::io;
use std::time::Instant;

use serde::Serialize;
use sshpick_core::{Entry, Picker, Preview, Resolution};
use sshpick_ui::ColorConfig;

use super::{CommandBase, HostView, did_you_mean, write_json};
use crate::cli;
use crate::cli::Exit;

const LISTED: usize = 5;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("{count} hosts match \"{query}\"; inspect needs exactly one")]
    Several {
        count: usize,
        query: String,
        names: Vec<String>,
    },
    #[error(transparent)]
    Resolve(#[from] sshpick_core::ResolveError),
    #[error("cannot write the result: {0}")]
    Write(#[source] io::Error),
}

impl Error {
    pub(crate) fn fix(&self) -> Option<String> {
        match self {
            Error::Several { query, names, .. } => {
                let shown = names
                    .iter()
                    .take(LISTED)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                let more = match names.len().saturating_sub(LISTED) {
                    0 => String::new(),
                    left => format!(" and {left} more"),
                };
                Some(format!(
                    "name one of {shown}{more}, or list them with `sshpick ls {query}`."
                ))
            }
            Error::Resolve(sshpick_core::ResolveError::Spawn(error))
                if error.kind() == io::ErrorKind::NotFound =>
            {
                Some("install OpenSSH, or put `ssh` on your PATH.".to_owned())
            }
            Error::Resolve(_) | Error::Write(_) => None,
        }
    }
}

#[derive(Serialize)]
struct View<'a> {
    #[serde(flatten)]
    host: HostView<'a>,
    resolved: Resolved<'a>,
    command: [&'a str; 3],
}

/// ssh's own key names.
#[derive(Serialize)]
struct Resolved<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    remotecommand: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxyjump: Option<&'a str>,
}

impl<'a> Resolved<'a> {
    fn of(preview: &'a Preview) -> Resolved<'a> {
        let get = |key: &str| {
            preview
                .lines()
                .find_map(|(name, value)| (name == key).then_some(value))
        };
        Resolved {
            user: get("user"),
            hostname: get("hostname"),
            port: get("port"),
            remotecommand: get("remotecommand"),
            proxyjump: get("proxyjump"),
        }
    }
}

pub(crate) fn run(
    base: &CommandBase,
    query: &str,
    json: bool,
    typed: &str,
) -> Result<u8, cli::Error> {
    let loaded = base.load()?;
    base.warn_all(&loaded.warnings);
    if !json {
        base.note_if_also_a_host(typed, &loaded.entries);
    }
    let picker = Picker::new(loaded.entries, query);
    let listing = picker.listing();
    let (host, entry) = match (picker.resolve(), listing.first()) {
        (Resolution::One(host), Some(entry)) => (host, *entry),
        (Resolution::Many(count), _) => {
            return Err(Error::Several {
                count,
                query: query.to_owned(),
                names: listing.iter().map(|entry| entry.host.to_string()).collect(),
            }
            .into());
        }
        _ => {
            return Err(cli::Error::NoMatch {
                query: query.to_owned(),
                did_you_mean: did_you_mean(query),
            });
        }
    };

    let started = Instant::now();
    let preview = sshpick_core::resolve(host).map_err(Error::Resolve)?;
    let command = sshpick_core::command(host);
    let output = base.output;
    let mut out = output.result();
    if json {
        let view = View {
            host: HostView::of(entry),
            resolved: Resolved::of(&preview),
            command,
        };
        write_json(out, &view).map_err(Error::Write)?;
    } else {
        output.log(&format!(
            "Resolved \"{host}\" with ssh -G [{}ms]",
            started.elapsed().as_millis()
        ));
        let text = render(entry, &preview, &command, output.stdout_colors());
        io::Write::write_all(&mut out, text.as_bytes())
            .and_then(|()| io::Write::flush(&mut out))
            .map_err(Error::Write)?;
    }
    Ok(Exit::Success.code())
}

fn render(entry: &Entry, preview: &Preview, command: &[&str], colors: ColorConfig) -> String {
    let names = entry.names().collect::<Vec<_>>().join(", ");
    let general: Vec<(&str, &str)> = [
        ("host", entry.host.as_str()),
        ("env", entry.env.as_str()),
        ("what", entry.what.as_str()),
        ("names", names.as_str()),
    ]
    .into_iter()
    .filter(|(key, value)| !value.is_empty() || *key == "host")
    .collect();
    let resolved: Vec<(&str, &str)> = preview.lines().collect();
    let command = command.join(" ");

    let mut blocks = vec![block("General", &general, colors)];
    if !resolved.is_empty() {
        blocks.push(block("Resolved", &resolved, colors));
    }
    blocks.push(format!("  {}\n\n    {command}\n", colors.bold("Command")));
    blocks.join("\n")
}

fn block(title: &str, rows: &[(&str, &str)], colors: ColorConfig) -> String {
    let widest = rows.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
    let mut text = format!("  {}\n\n", colors.bold(title));
    for (key, value) in rows {
        let pad = " ".repeat(widest - key.len() + 2);
        text.push_str(&format!("    {}{pad}{value}\n", colors.dim(key)));
    }
    text
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;

    fn entry(host: &str) -> Entry {
        let home = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/words");
        let entries = sshpick_core::load(&home).unwrap().entries;
        entries
            .into_iter()
            .find(|entry| entry.host.as_str() == host)
            .unwrap()
    }

    fn preview() -> Preview {
        Preview::parse("user ubuntu\nhostname 192.0.2.11\nport 22\nproxyjump stg-bastion\n")
    }

    #[test]
    fn the_text_has_three_blocks_with_keys_padded_to_the_longest_in_each() {
        let entry = entry("stg-consul1");
        let command = sshpick_core::command(&entry.host);
        let text = render(&entry, &preview(), &command, ColorConfig::none());
        assert_snapshot!("inspect_text", text);
        assert!(text.lines().all(|line| !line.ends_with(' ')));
    }

    #[test]
    fn empty_env_and_what_are_left_out_and_no_resolved_block_means_no_keys() {
        let entry = entry("web");
        let command = sshpick_core::command(&entry.host);
        let text = render(&entry, &Preview::parse(""), &command, ColorConfig::none());
        assert_snapshot!("inspect_text_bare", text);
    }

    #[test]
    fn colour_dims_the_keys_and_bolds_the_titles() {
        let entry = entry("list");
        let command = sshpick_core::command(&entry.host);
        let colors = ColorConfig::new(true, false, &|_| None);
        let text = render(&entry, &preview(), &command, colors);
        assert!(text.contains("  \x1b[1mGeneral\x1b[22m\n"));
        assert!(text.contains("    \x1b[2mhost\x1b[22m   list\n"));
    }

    #[test]
    fn the_json_resolved_object_keeps_only_the_keys_ssh_printed() {
        let entry = entry("web");
        let preview = preview();
        let view = View {
            host: HostView::of(&entry),
            resolved: Resolved::of(&preview),
            command: sshpick_core::command(&entry.host),
        };
        let mut out = Vec::new();
        write_json(&mut out, &view).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            value["resolved"],
            serde_json::json!({
                "user": "ubuntu",
                "hostname": "192.0.2.11",
                "port": "22",
                "proxyjump": "stg-bastion",
            })
        );
        assert_eq!(value["command"], serde_json::json!(["ssh", "--", "web"]));
        assert_eq!(value["names"], serde_json::json!(["web", "ls"]));
        assert_eq!(value["env"], "");
    }

    #[test]
    fn the_fix_names_up_to_five_hosts_and_counts_the_rest() {
        let several = |count: usize| Error::Several {
            count,
            query: "web".to_owned(),
            names: (1..=count).map(|n| format!("web{n}")).collect(),
        };
        assert_eq!(
            several(3).fix().as_deref(),
            Some("name one of web1, web2, web3, or list them with `sshpick ls web`.")
        );
        assert_eq!(
            several(7).fix().as_deref(),
            Some(
                "name one of web1, web2, web3, web4, web5 and 2 more, or list them with `sshpick ls web`."
            )
        );
    }
}
