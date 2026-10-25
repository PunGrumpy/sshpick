use std::ffi::OsStr;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
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

/// Where a host goes, in the pieces of `user@hostname:port via jump runs cmd` so that the UI can
/// style the joining words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route<'a> {
    pub user: Option<&'a str>,
    pub hostname: &'a str,
    pub port: Option<&'a str>,
    pub via: Option<&'a str>,
    pub runs: Option<&'a str>,
}

impl Route<'_> {
    pub fn target(&self) -> String {
        let user = self.user.map(|user| format!("{user}@"));
        let port = self.port.map(|port| format!(":{port}"));
        format!(
            "{}{}{}",
            user.unwrap_or_default(),
            self.hostname,
            port.unwrap_or_default()
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("cannot run ssh -G: {0}")]
    Spawn(#[source] io::Error),
    /// No exit within [`TIMEOUT`]; the child was killed.
    #[error("ssh -G gave no answer within {} s", TIMEOUT.as_secs())]
    Timeout,
    /// `ssh -G` exited non-zero (`code` is `None` if a signal killed it). `stderr` is what it
    /// said. One bad option anywhere in the config fails every host this way.
    #[error("{}", exit_message(.code, .stderr))]
    Ssh { code: Option<i32>, stderr: String },
}

impl ResolveError {
    pub fn is_transient(&self) -> bool {
        matches!(self, ResolveError::Timeout)
    }

    pub fn headline(&self) -> String {
        let said = match self {
            ResolveError::Ssh { stderr, .. } => {
                first_line(stderr).map(|line| format!("ssh -G failed: {line}"))
            }
            ResolveError::Spawn(_) | ResolveError::Timeout => None,
        };
        let text = said.unwrap_or_else(|| self.to_string());
        first_line(&text)
            .unwrap_or_default()
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect()
    }
}

fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

impl Preview {
    pub fn parse(stdout: &str) -> Preview {
        parse_g(stdout)
    }

    pub fn lines(&self) -> impl Iterator<Item = (&'static str, &str)> {
        self.0.iter().map(|(key, value)| (*key, value.as_str()))
    }

    pub fn route(&self) -> Option<Route<'_>> {
        let get = |key: &str| {
            self.lines()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.trim())
                .filter(|value| !value.is_empty())
        };
        Some(Route {
            user: get("user"),
            hostname: get("hostname")?,
            port: get("port"),
            via: get("proxyjump"),
            runs: get("remotecommand"),
        })
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
/// picker's block would pass straight to ssh.
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
    let mut command = Command::new(program);
    command.args(["-G", "--"]).arg(host.as_str());
    let Finished {
        status,
        stdout,
        stderr,
    } = run(command, timeout).map_err(|error| match error {
        RunError::Spawn(error) => ResolveError::Spawn(error),
        RunError::Timeout => ResolveError::Timeout,
    })?;
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

/// The first line `ssh -V` prints.
pub fn version() -> io::Result<String> {
    version_with(OsStr::new(SSH), TIMEOUT)
}

pub(crate) fn version_with(program: &OsStr, timeout: Duration) -> io::Result<String> {
    let mut command = Command::new(program);
    command.arg("-V");
    let finished = run(command, timeout).map_err(|error| match error {
        RunError::Spawn(error) => error,
        RunError::Timeout => io::Error::new(
            io::ErrorKind::TimedOut,
            format!("no answer within {} s", TIMEOUT.as_secs()),
        ),
    })?;
    if !finished.status.success() {
        return Err(io::Error::other(match finished.status.code() {
            Some(code) => format!("exited with {code}"),
            None => "was killed by a signal".to_owned(),
        }));
    }
    version_line(&finished.stderr, &finished.stdout)
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other("printed nothing"))
}

/// OpenSSH prints its version on stderr; a wrapper that prints it on stdout is read too.
fn version_line<'a>(stderr: &'a str, stdout: &'a str) -> Option<&'a str> {
    first_line(stderr).or_else(|| first_line(stdout))
}

struct Finished {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

enum RunError {
    Spawn(io::Error),
    Timeout,
}

fn run(mut command: Command, timeout: Duration) -> Result<Finished, RunError> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(RunError::Spawn)?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                reap(&mut child);
                return Err(RunError::Timeout);
            }
            Ok(None) => thread::sleep(POLL),
            Err(error) => {
                reap(&mut child);
                return Err(RunError::Spawn(error));
            }
        }
    };
    Ok(Finished {
        status,
        stdout: String::from_utf8_lossy(&stdout.join().unwrap_or_default()).into_owned(),
        stderr: String::from_utf8_lossy(&stderr.join().unwrap_or_default()).into_owned(),
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

    use test_case::test_case;

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
    fn the_route_is_picked_by_key_whatever_order_ssh_printed() {
        let preview = Preview::parse(
            "proxyjump stg-bastion\nport 2222\nremotecommand sudo -i\nhostname 10.0.0.5\nuser ops\n",
        );
        assert_eq!(
            preview.route(),
            Some(Route {
                user: Some("ops"),
                hostname: "10.0.0.5",
                port: Some("2222"),
                via: Some("stg-bastion"),
                runs: Some("sudo -i"),
            })
        );
    }

    #[test]
    fn the_target_is_user_at_hostname_colon_port_with_what_is_there() {
        let route = |user, port| Route {
            user,
            hostname: "10.0.0.5",
            port,
            via: Some("ignored"),
            runs: Some("ignored"),
        };
        assert_eq!(route(Some("ops"), Some("22")).target(), "ops@10.0.0.5:22");
        assert_eq!(route(None, Some("22")).target(), "10.0.0.5:22");
        assert_eq!(route(Some("ops"), None).target(), "ops@10.0.0.5");
        assert_eq!(route(None, None).target(), "10.0.0.5");
    }

    #[test]
    fn the_route_keeps_the_default_port_and_leaves_out_what_ssh_did_not_print() {
        let preview = Preview::parse("user ubuntu\nhostname 192.0.2.11\nport 22\n");
        assert_eq!(
            preview.route(),
            Some(Route {
                user: Some("ubuntu"),
                hostname: "192.0.2.11",
                port: Some("22"),
                via: None,
                runs: None,
            })
        );
    }

    #[test]
    fn there_is_no_route_without_a_hostname_and_an_empty_piece_is_absent() {
        assert_eq!(Preview::parse("user ubuntu\nport 22\n").route(), None);
        assert_eq!(Preview::parse("").route(), None);
        let preview = Preview::parse("user \nhostname a\nproxyjump  \n");
        assert_eq!(
            preview.route(),
            Some(Route {
                user: None,
                hostname: "a",
                port: None,
                via: None,
                runs: None,
            })
        );
    }

    #[test]
    fn the_headline_is_the_first_line_ssh_said() {
        let ssh = |stderr: &str| ResolveError::Ssh {
            code: Some(255),
            stderr: stderr.to_owned(),
        };
        assert_eq!(
            ssh("/home/me/.ssh/config line 12: Bad configuration option: foo\n/home/me/.ssh/config: terminating, 1 bad configuration options").headline(),
            "ssh -G failed: /home/me/.ssh/config line 12: Bad configuration option: foo"
        );
        assert_eq!(
            ssh("\n  \n  second  \nthird").headline(),
            "ssh -G failed: second",
            "blank lines and indentation are skipped"
        );
        assert_eq!(
            ssh("bad \x1b[31moption\x07\tx").headline(),
            "ssh -G failed: bad  [31moption  x"
        );
    }

    #[test]
    fn the_headline_reads_as_the_other_failures_already_do() {
        let silent = ResolveError::Ssh {
            code: Some(255),
            stderr: String::new(),
        };
        assert_eq!(silent.headline(), "ssh -G exited with 255");
        let killed = ResolveError::Ssh {
            code: None,
            stderr: String::new(),
        };
        assert_eq!(killed.headline(), "ssh -G was killed by a signal");
        assert_eq!(
            ResolveError::Timeout.headline(),
            "ssh -G gave no answer within 2 s"
        );
        let spawn = ResolveError::Spawn(io::Error::other("no such file\nor directory"));
        assert_eq!(spawn.headline(), "cannot run ssh -G: no such file");
    }

    #[test_case("OpenSSH_9.6p1, OpenSSL 3.0.13\n", "", "OpenSSH_9.6p1, OpenSSL 3.0.13"; "stderr")]
    #[test_case("\n  OpenSSH_9.6p1  \nsecond\n", "", "OpenSSH_9.6p1"; "blank lines and indentation are skipped")]
    #[test_case("", "OpenSSH_9.6p1\n", "OpenSSH_9.6p1"; "stdout when stderr says nothing")]
    #[test_case("from stderr", "from stdout", "from stderr"; "stderr first")]
    #[test_case(" \n", "\n", ""; "nothing printed")]
    fn the_version_is_the_first_line_ssh_printed(stderr: &str, stdout: &str, want: &str) {
        assert_eq!(version_line(stderr, stdout).unwrap_or_default(), want);
    }

    #[test]
    fn version_asks_ssh_dash_v_and_returns_its_first_line() {
        let line = version_with(fake("ssh-ok").as_os_str(), PATIENT).unwrap();
        assert_eq!(
            line,
            "OpenSSH_9.6p1 Ubuntu-3ubuntu13.5, OpenSSL 3.0.13 30 Jan 2024"
        );
    }

    #[test]
    fn version_reports_a_missing_program_with_its_own_error_kind() {
        let error =
            version_with(OsStr::new("/nonexistent/ssh"), Duration::from_secs(1)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn version_reports_a_failing_ssh_by_its_exit_status() {
        let error = version_with(fake("ssh-bad-config").as_os_str(), PATIENT).unwrap_err();
        assert_eq!(error.to_string(), "exited with 255");
    }

    #[test]
    fn version_kills_a_hung_ssh_at_the_deadline() {
        let started = Instant::now();
        let error =
            version_with(fake("ssh-hang").as_os_str(), Duration::from_millis(100)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(error.to_string(), "no answer within 2 s");
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "{:?}",
            started.elapsed()
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
    fn a_timed_out_ssh_is_killed_and_reaped() {
        let dir = std::env::temp_dir().join(format!("sshpick-reap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pid_file = dir.join("pid");
        let error = resolve_with(
            fake("ssh-hang-pid").as_os_str(),
            &host(pid_file.to_str().unwrap()),
            Duration::from_millis(500),
        )
        .unwrap_err();
        assert!(matches!(error, ResolveError::Timeout), "{error:?}");
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let alive = Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(!alive, "pid {} outlived the timeout", pid.trim());
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

    #[test]
    fn only_a_timeout_is_transient() {
        let cases = [
            (ResolveError::Timeout, true),
            (ResolveError::Spawn(io::Error::other("no ssh")), false),
            (
                ResolveError::Ssh {
                    code: Some(255),
                    stderr: "Bad port".to_owned(),
                },
                false,
            ),
            (
                ResolveError::Ssh {
                    code: None,
                    stderr: String::new(),
                },
                false,
            ),
        ];
        for (error, want) in cases {
            assert_eq!(error.is_transient(), want, "{error}");
        }
    }
}
