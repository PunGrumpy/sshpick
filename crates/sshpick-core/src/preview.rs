use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::config::Host;
use crate::ssh::{self, Preview, ResolveError, SSH, TIMEOUT};

#[derive(Debug)]
pub struct Previews {
    requests: Sender<Host>,
    replies: Receiver<Reply>,
    slots: HashMap<Host, Slot>,
    shown: Option<Host>,
}

#[derive(Debug)]
pub enum Slot {
    Waiting,
    Ready(Preview),
    /// Kept for the session (a broken config would otherwise spawn a failing `ssh` per key),
    /// except [`ResolveError::Timeout`], which is asked again on the host's next arrival.
    Failed(ResolveError),
}

#[derive(Debug)]
enum Reply {
    Resolved(Host, Result<Preview, ResolveError>),
    Superseded(Host),
}

impl Previews {
    /// Start the worker. Fails only if the OS refuses a thread.
    pub fn spawn() -> io::Result<Previews> {
        Previews::start(OsString::from(SSH), TIMEOUT)
    }

    fn start(program: OsString, timeout: Duration) -> io::Result<Previews> {
        let (requests, inbox) = mpsc::channel();
        let (outbox, replies) = mpsc::channel();
        thread::Builder::new()
            .name("ssh-G".to_owned())
            .spawn(move || serve(&inbox, &outbox, &program, timeout))?;
        Ok(Previews {
            requests,
            replies,
            slots: HashMap::new(),
            shown: None,
        })
    }

    /// The slot for `host`, asking the worker first if it has none. Call it once per frame for
    /// the selected host; it never blocks.
    pub fn fetch(&mut self, host: &Host) -> &Slot {
        self.drain();
        if self.shown.as_ref() != Some(host) {
            if let Some(Slot::Failed(ResolveError::Timeout)) = self.slots.get(host) {
                self.slots.remove(host);
            }
            self.shown = Some(host.clone());
        }
        let requests = &self.requests;
        self.slots
            .entry(host.clone())
            .or_insert_with(|| match requests.send(host.clone()) {
                Ok(()) => Slot::Waiting,
                Err(_) => Slot::Failed(ResolveError::Spawn(io::Error::other(
                    "the preview worker stopped",
                ))),
            })
    }

    pub fn prefetch(&mut self, host: &Host) {
        self.fetch(host);
    }

    pub fn busy(&self) -> bool {
        self.slots
            .values()
            .any(|slot| matches!(slot, Slot::Waiting))
    }

    fn drain(&mut self) {
        while let Ok(reply) = self.replies.try_recv() {
            match reply {
                Reply::Resolved(host, Ok(preview)) => self.slots.insert(host, Slot::Ready(preview)),
                Reply::Resolved(host, Err(error)) => self.slots.insert(host, Slot::Failed(error)),
                Reply::Superseded(host) => self.slots.remove(&host),
            };
        }
    }
}

fn serve(inbox: &Receiver<Host>, outbox: &Sender<Reply>, program: &OsStr, timeout: Duration) {
    while let Ok(mut host) = inbox.recv() {
        while let Ok(newer) = inbox.try_recv() {
            if outbox.send(Reply::Superseded(host)).is_err() {
                return;
            }
            host = newer;
        }
        let result = ssh::resolve_with(program, &host, timeout);
        if outbox.send(Reply::Resolved(host, result)).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    use super::*;

    fn fake(name: &str) -> OsString {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bin")
            .join(name);
        path.into_os_string()
    }

    fn host(name: &str) -> Host {
        Host::literal(name).unwrap()
    }

    const PATIENT: Duration = Duration::from_secs(30);

    fn kind(slot: &Slot) -> &'static str {
        match slot {
            Slot::Waiting => "waiting",
            Slot::Ready(_) => "ready",
            Slot::Failed(ResolveError::Timeout) => "timeout",
            Slot::Failed(ResolveError::Ssh { .. }) => "ssh",
            Slot::Failed(ResolveError::Spawn(_)) => "spawn",
        }
    }

    fn settled(previews: &mut Previews, host: &Host) -> &'static str {
        let give_up = Instant::now() + Duration::from_secs(60);
        loop {
            let seen = kind(previews.fetch(host));
            if seen != "waiting" || Instant::now() > give_up {
                return seen;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn every_request_gets_one_reply() {
        let (requests, inbox) = mpsc::channel();
        let (outbox, replies) = mpsc::channel();
        for name in ["a", "b", "c"] {
            requests.send(host(name)).unwrap();
        }
        drop(requests);
        serve(&inbox, &outbox, &fake("ssh-ok"), PATIENT);
        let replies: Vec<Reply> = replies.try_iter().collect();
        let [
            Reply::Superseded(a),
            Reply::Superseded(b),
            Reply::Resolved(c, Ok(preview)),
        ] = replies.as_slice()
        else {
            panic!("unexpected replies: {replies:?}");
        };
        assert_eq!((a.as_str(), b.as_str(), c.as_str()), ("a", "b", "c"));
        assert_eq!(preview.lines().count(), 4);

        let mut previews = Previews::start(fake("ssh-ok"), PATIENT).unwrap();
        for name in ["a", "b"] {
            previews.fetch(&host(name));
        }
        assert_eq!(settled(&mut previews, &host("c")), "ready");
        assert!(!previews.busy());
    }

    #[test]
    fn ssh_errors_are_cached_and_timeouts_retried_on_arrival() {
        let (a, b) = (host("a"), host("b"));
        let mut previews = Previews::start(fake("ssh-bad-config"), PATIENT).unwrap();
        assert_eq!(settled(&mut previews, &a), "ssh");
        assert_eq!(settled(&mut previews, &b), "ssh");
        assert_eq!(
            kind(previews.fetch(&a)),
            "ssh",
            "an ssh error is kept, not asked again"
        );
        assert!(!previews.busy());

        let mut previews = Previews::start(fake("ssh-hang"), Duration::from_millis(50)).unwrap();
        assert_eq!(settled(&mut previews, &a), "timeout");
        assert_eq!(
            kind(previews.fetch(&a)),
            "timeout",
            "unchanged while the cursor stays"
        );
        assert!(!previews.busy());
        assert_eq!(settled(&mut previews, &b), "timeout");
        assert_eq!(
            kind(previews.fetch(&a)),
            "waiting",
            "asked again when the cursor returns"
        );
        assert!(previews.busy());
        assert_eq!(settled(&mut previews, &a), "timeout");
    }

    #[test]
    fn a_missing_program_fails_the_slot_once() {
        let mut previews = Previews::start(OsString::from("/nonexistent/ssh"), PATIENT).unwrap();
        let a = host("a");
        assert_eq!(settled(&mut previews, &a), "spawn");
        assert_eq!(kind(previews.fetch(&a)), "spawn");
    }
}
