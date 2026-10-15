use std::ffi::OsStr;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config::Host;

pub(crate) const SSH: &str = "ssh";

const POLL: Duration = Duration::from_millis(5);

/// What `ssh -G` prints on stderr on every call (`RequestTTY`), whatever the exit status.
const TTY_NOTICE: &str = "Pseudo-terminal will not be allocated";

pub(crate) const TIMEOUT: Duration = Duration::from_secs(2);

const KEYS: [&str; 5] = ["user", "hostname", "port", "remotecommand", "proxyjump"];

/// Where `ssh -G` says a host resolves: the shown keys in ssh's output order (user, hostname,
/// port, remotecommand, proxyjump). A key ssh does not print is absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview(Vec<(&'static str, String)>);

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("cannot run ssh -G: {0}")]
    Spawn(#[source] io::Error),
    /// No exit within [`TIMEOUT`]; the child was killed. The pane retries when the cursor comes
    /// back (load spikes pass; see `Previews::fetch`).
    #[error("ssh -G gave no answer within {} s", TIMEOUT.as_secs())]
    Timeout,
    /// `ssh -G` exited non-zero (`code` is `None` if a signal killed it). `stderr` is what it
    /// said. One bad option anywhere in the config fails every host this way.
    #[error("{}", exit_message(.code, .stderr))]
    Ssh { code: Option<i32>, stderr: String },
}

impl Preview {
    pub fn parse(stdout: &str) -> Preview {
        parse_g(stdout)
    }

    pub fn lines(&self) -> impl Iterator<Item = (&'static str, &str)> {
        self.0.iter().map(|(key, value)| (*key, value.as_str()))
    }
}

fn exit_message(code: &Option<i32>, stderr: &str) -> String {
    match code {
        _ if !stderr.is_empty() => stderr.to_owned(),
        Some(code) => format!("ssh -G exited with {code}"),
        None => "ssh -G was killed by a signal".to_owned(),
    }
}

/// The argv of the connection: `ssh -- <host>`.
pub fn command(host: &Host) -> [&str; 3] {
    [SSH, "--", host.as_str()]
}

/// Replace this process with `ssh -- <host>`. Returns only if exec failed.
///
/// The caller restores the terminal first: exec runs no destructors, so raw mode and the
/// alternate screen would pass straight to ssh.
pub fn exec(host: &Host) -> io::Error {
    let [program, dashes, name] = command(host);
    Command::new(program).args([dashes, name]).exec()
}

pub fn resolve(host: &Host) -> Result<Preview, ResolveError> {
    resolve_with(OsStr::new(SSH), host, TIMEOUT)
}

pub(crate) fn resolve_with(
    program: &OsStr,
    host: &Host,
    timeout: Duration,
) -> Result<Preview, ResolveError> {
    let mut child = Command::new(program)
        .args(["-G", "--"])
        .arg(host.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(ResolveError::Spawn)?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                reap(&mut child);
                return Err(ResolveError::Timeout);
            }
            Ok(None) => thread::sleep(POLL),
            Err(error) => {
                reap(&mut child);
                return Err(ResolveError::Spawn(error));
            }
        }
    };
    let stdout = String::from_utf8_lossy(&stdout.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&stderr.join().unwrap_or_default()).into_owned();
    if status.success() {
        return Ok(parse_g(&stdout));
    }
    let said: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(TTY_NOTICE))
        .collect();
    Err(ResolveError::Ssh {
        code: status.code(),
        stderr: said.join("\n"),
    })
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    })
}

fn reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn parse_g(stdout: &str) -> Preview {
    let lines = stdout.lines().filter_map(|line| {
        let (word, value) = line.split_once(' ')?;
        let key = KEYS.iter().find(|key| **key == word)?;
        Some((*key, value.to_owned()))
    });
    Preview(lines.collect())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn fake(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bin")
            .join(name)
    }

    fn host(name: &str) -> Host {
        Host::literal(name).unwrap()
    }

    const PATIENT: Duration = Duration::from_secs(30);

    #[test]
    fn parse_g_keeps_five_keys_in_ssh_order() {
        let stdout = "host oldstg-db\nuser ubuntu\nhostname 183.90.170.152\nport 12345\n\
                      addressfamily any\nremotecommand sudo ssh -t ubuntu@10.148.0.31\n";
        let preview = parse_g(stdout);
        let got: Vec<_> = preview.lines().collect();
        assert_eq!(
            got,
            [
                ("user", "ubuntu"),
                ("hostname", "183.90.170.152"),
                ("port", "12345"),
                ("remotecommand", "sudo ssh -t ubuntu@10.148.0.31"),
            ]
        );
    }

    #[test]
    fn the_command_is_ssh_double_dash_host() {
        assert_eq!(command(&host("stg-consul1")), ["ssh", "--", "stg-consul1"]);
    }

    #[test]
    fn parse_is_the_public_face_of_parse_g() {
        let stdout = "user ubuntu\nport 22\n";
        assert_eq!(Preview::parse(stdout), parse_g(stdout));
    }

    #[test]
    fn parse_g_ignores_other_keys_and_bare_words() {
        let preview = parse_g("hostnamex a\nuserknownhostsfile x\nport\nproxyjump a, b\r\n");
        let got: Vec<_> = preview.lines().collect();
        assert_eq!(got, [("proxyjump", "a, b")]);
        assert_eq!(parse_g("").lines().count(), 0);
    }

    #[test]
    fn resolve_passes_double_dash_and_resolves() {
        let preview = resolve_with(fake("ssh-ok").as_os_str(), &host("web-1"), PATIENT).unwrap();
        let got: Vec<_> = preview.lines().collect();
        assert_eq!(
            got,
            [
                ("user", "ubuntu"),
                ("hostname", "192.0.2.10"),
                ("port", "22"),
                ("proxyjump", "new-bastion"),
            ]
        );
    }

    #[test]
    fn resolve_reports_ssh_errors_without_the_tty_notice() {
        let error =
            resolve_with(fake("ssh-bad-config").as_os_str(), &host("web-1"), PATIENT).unwrap_err();
        let ResolveError::Ssh { code, stderr } = &error else {
            panic!("expected Ssh, got {error:?}");
        };
        assert_eq!(*code, Some(255));
        assert_eq!(
            stderr,
            "/home/me/.ssh/config.d/bad.conf line 2: Invalid ProxyJump \"x:%p\"\n\
             /home/me/.ssh/config: terminating, 1 bad configuration options"
        );
        assert_eq!(error.to_string(), *stderr);
    }

    #[test]
    fn resolve_kills_a_hung_ssh_at_the_deadline() {
        let started = Instant::now();
        let error = resolve_with(
            fake("ssh-hang").as_os_str(),
            &host("web-1"),
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(matches!(error, ResolveError::Timeout), "{error:?}");
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn resolve_reports_a_missing_program() {
        let error = resolve_with(
            OsStr::new("/nonexistent/ssh"),
            &host("web-1"),
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(matches!(error, ResolveError::Spawn(_)), "{error:?}");
        assert!(error.to_string().starts_with("cannot run ssh -G: "));
        assert!(std::error::Error::source(&error).is_some());
        assert!(std::error::Error::source(&ResolveError::Timeout).is_none());
    }

    #[test]
    fn errors_read_as_one_message() {
        assert_eq!(
            ResolveError::Timeout.to_string(),
            "ssh -G gave no answer within 2 s"
        );
        let silent = ResolveError::Ssh {
            code: Some(255),
            stderr: String::new(),
        };
        assert_eq!(silent.to_string(), "ssh -G exited with 255");
        let loud = ResolveError::Ssh {
            code: Some(255),
            stderr: "Bad port".to_owned(),
        };
        assert_eq!(
            loud.to_string(),
            "Bad port",
            "stderr wins over the exit code"
        );
        let killed = ResolveError::Ssh {
            code: None,
            stderr: String::new(),
        };
        assert_eq!(killed.to_string(), "ssh -G was killed by a signal");
    }
}
