//! Which hosts were connected to, and when. A connection appends one line. Nothing is ever
//! rewritten: the file rotates like a log, and the reader keeps the latest use per host.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{Entry, Host, shown};
use crate::interaction::{Interaction, refusal};

const DIR: &str = "sshpick";
const FILE: &str = "history";
const OLD: &str = "history.old";
/// `history` is renamed to `history.old` once it is longer than this, so the two files together
/// stay under twice this. A line is about 26 bytes, so one file holds about 2,500 connections.
const LIMIT: u64 = 64 * 1024;
/// A bloated file is read from its end, so the picker never waits on it.
const READ: u64 = 2 * LIMIT;

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;
const YEAR: u64 = 365 * DAY;
/// Below the first number of seconds, count in the second, spelled with the letter.
const AGES: [(u64, u64, char); 4] = [
    (HOUR, MINUTE, 'm'),
    (DAY, HOUR, 'h'),
    (2 * WEEK, DAY, 'd'),
    (YEAR, WEEK, 'w'),
];

/// Where connections are remembered, or nowhere.
#[derive(Debug)]
pub struct History {
    /// `None` reads nothing and writes nothing.
    dir: Option<PathBuf>,
}

/// When each host was last connected to, as of one instant. Immutable once built.
#[derive(Debug, Default)]
pub struct Recents {
    /// Unix seconds.
    now: u64,
    /// A name as it was recorded, to the latest time it was recorded. Keys are lookup text only:
    /// no `Host` is ever built from one, so nothing in the file can reach `ssh`.
    last: HashMap<String, u64>,
}

/// One host's latest connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Last {
    /// Unix seconds, never later than the `Recents` it came from: a time in the future counts
    /// as now.
    pub(crate) at: u64,
    /// `now`, `5m`, `2h`, `3d`, `6w` or `1y`. At most three cells.
    pub(crate) ago: String,
}

#[derive(Debug, thiserror::Error)]
#[error("cannot save recent hosts to {}: {source}", shown(.path))]
pub struct RecordError {
    path: PathBuf,
    source: io::Error,
}

impl History {
    /// History is kept for a person at a terminal: exactly when the picker could open, which is
    /// when `refusal(interaction)` is `None`. Under `--non-interactive`, a pipe or `TERM=dumb`
    /// the result reads nothing and writes nothing.
    ///
    /// The directory is `$XDG_STATE_HOME/sshpick` when that variable is an absolute path, else
    /// `<home>/.local/state/sshpick`.
    pub fn open(home: &Path, interaction: &Interaction) -> History {
        History::open_with(home, interaction, &|name| std::env::var_os(name))
    }

    pub(crate) fn open_with(
        home: &Path,
        interaction: &Interaction,
        var: &dyn Fn(&str) -> Option<OsString>,
    ) -> History {
        if refusal(interaction).is_some() {
            return History { dir: None };
        }
        let state = var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".local").join("state"));
        History {
            dir: Some(state.join(DIR)),
        }
    }

    /// The latest use of each recorded name, as of `now`. Never fails: a missing or unreadable
    /// file is no history, and a line that does not parse is skipped.
    pub fn recents(&self, now: SystemTime) -> Recents {
        let Some(dir) = &self.dir else {
            return Recents::none();
        };
        // `history` first: a rotation between the two reads then shows a line twice, and never
        // skips a generation.
        let logs = [FILE, OLD].map(|name| tail(&dir.join(name)));
        Recents::collect(logs.iter().flat_map(|log| parse(log)), seconds(now))
    }

    /// Remember that `host` was chosen at `now`.
    ///
    /// One line, `<unix seconds>\t<host>\n`, in a single `write` on a file opened for append. The
    /// `Host` invariant (no whitespace, no control character) is what makes the line well formed.
    /// The directory is created `0700` and the file `0600`.
    pub fn record(&self, host: &Host, now: SystemTime) -> Result<(), RecordError> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        let fail = |path: &Path| {
            let path = path.to_owned();
            move |source| RecordError { path, source }
        };
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(fail(dir))?;
        let path = dir.join(FILE);
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(&path)
            .map_err(fail(&path))?;
        let line = format!("{}\t{host}\n", seconds(now));
        file.write_all(line.as_bytes()).map_err(fail(&path))?;
        let long = |meta: io::Result<fs::Metadata>| meta.is_ok_and(|meta| meta.len() > LIMIT);
        if long(file.metadata()) && long(fs::metadata(&path)) {
            // Ignored: the next connection tries again.
            let _ = fs::rename(&path, dir.join(OLD));
        }
        Ok(())
    }
}

impl Recents {
    /// No history: every host ranks as it does today.
    pub fn none() -> Recents {
        Recents::default()
    }

    /// These names were last used at these times. For a repeated name the latest time wins.
    pub fn of<'a>(
        uses: impl IntoIterator<Item = (&'a str, SystemTime)>,
        now: SystemTime,
    ) -> Recents {
        let uses = uses.into_iter().map(|(name, at)| (name, seconds(at)));
        Recents::collect(uses, seconds(now))
    }

    fn collect<'a>(uses: impl Iterator<Item = (&'a str, u64)>, now: u64) -> Recents {
        let mut last: HashMap<String, u64> = HashMap::new();
        for (name, at) in uses {
            match last.get_mut(name) {
                Some(latest) => *latest = (*latest).max(at),
                None => {
                    last.insert(name.to_owned(), at);
                }
            }
        }
        Recents { now, last }
    }

    /// The latest use of any name on the entry's `Host` line, so a host keeps its history when
    /// the name shown changes but the old one stays on the line.
    pub(crate) fn last(&self, entry: &Entry) -> Option<Last> {
        let at = entry
            .names()
            .filter_map(|name| self.last.get(name))
            .max()?
            .min(&self.now);
        Some(Last {
            at: *at,
            ago: ago(self.now - at),
        })
    }
}

fn seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The last `READ` bytes of the file as text. A file that is missing, a directory or unreadable
/// reads as empty.
fn tail(path: &Path) -> String {
    read_tail(path).unwrap_or_default()
}

fn read_tail(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let skipped = file.metadata()?.len().saturating_sub(READ);
    file.seek(SeekFrom::Start(skipped))?;
    let mut bytes = Vec::new();
    file.take(READ).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    if skipped == 0 {
        return Ok(text.into_owned());
    }
    // The cut can land inside a line.
    Ok(text
        .split_once('\n')
        .map_or_else(String::new, |(_, rest)| rest.to_owned()))
}

/// `(name, unix seconds)` for each line that is digits, one tab, then a name. Anything after a
/// second tab is ignored, which leaves room for a later field without a version line. A last line
/// with no newline was cut short, so it is skipped.
fn parse(log: &str) -> impl Iterator<Item = (&str, u64)> {
    log.split_inclusive('\n').filter_map(|line| {
        let (time, rest) = line.strip_suffix('\n')?.split_once('\t')?;
        let name = rest.split('\t').next().filter(|name| !name.is_empty())?;
        if !time.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        Some((name, time.parse().ok()?))
    })
}

/// How long ago, in at most three cells: under a minute is `now`, then `59m`, `23h`, `13d`,
/// `52w`, then years.
fn ago(seconds: u64) -> String {
    if seconds < MINUTE {
        return "now".to_owned();
    }
    let (count, unit) = AGES
        .iter()
        .find(|&&(below, ..)| seconds < below)
        .map_or(((seconds / YEAR).min(99), 'y'), |&(_, size, unit)| {
            (seconds / size, unit)
        });
    format!("{count}{unit}")
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use test_case::test_case;

    use super::*;

    const NOW: u64 = 1_794_556_800;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn host(name: &str) -> Host {
        Host::literal(name).unwrap()
    }

    fn entry(host: &str, aliases: &[&str]) -> Entry {
        Entry {
            env: String::new(),
            host: Host::literal(host).unwrap(),
            what: String::new(),
            aliases: aliases.iter().map(|alias| alias.to_string()).collect(),
            hostname: None,
            user: None,
            hop: None,
        }
    }

    fn terminal() -> Interaction {
        Interaction {
            non_interactive: false,
            stdin_is_terminal: true,
            stdout_is_terminal: true,
            dumb: false,
        }
    }

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(test: &str) -> Scratch {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "sshpick-history-{test}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Scratch(root)
        }

        fn history(&self) -> History {
            History {
                dir: Some(self.0.join(DIR)),
            }
        }

        fn file(&self, name: &str) -> PathBuf {
            self.0.join(DIR).join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn recent_names(history: &History) -> Vec<String> {
        let mut names: Vec<String> = history.recents(at(NOW)).last.into_keys().collect();
        names.sort();
        names
    }

    fn big(name: &str) -> String {
        format!("1\t{name}\n").repeat(usize::try_from(LIMIT).unwrap() / (name.len() + 3) + 1)
    }

    #[test_case(Some("/var/state"), "/var/state/sshpick"; "an absolute XDG_STATE_HOME")]
    #[test_case(None, "/home/me/.local/state/sshpick"; "unset")]
    #[test_case(Some(""), "/home/me/.local/state/sshpick"; "empty")]
    #[test_case(Some("state"), "/home/me/.local/state/sshpick"; "relative")]
    fn the_directory_follows_xdg_state_home(xdg: Option<&str>, want: &str) {
        let var = |name: &str| {
            let value = xdg.filter(|_| name == "XDG_STATE_HOME");
            value.map(OsString::from)
        };
        let history = History::open_with(Path::new("/home/me"), &terminal(), &var);
        assert_eq!(history.dir.as_deref(), Some(Path::new(want)));
    }

    #[test]
    fn only_a_picker_that_could_open_keeps_history() {
        let scratch = Scratch::new("refusals");
        let state = scratch.0.join("state");
        let var = |_: &str| Some(state.clone().into_os_string());
        for bits in 0..16u8 {
            let interaction = Interaction {
                non_interactive: bits & 1 != 0,
                stdin_is_terminal: bits & 2 != 0,
                stdout_is_terminal: bits & 4 != 0,
                dumb: bits & 8 != 0,
            };
            let history = History::open_with(&scratch.0, &interaction, &var);
            assert_eq!(
                history.dir.is_some(),
                refusal(&interaction).is_none(),
                "{interaction:?}"
            );
            if history.dir.is_none() {
                history.record(&host("lab-a"), at(NOW)).unwrap();
                assert!(history.recents(at(NOW)).last.is_empty());
                assert!(!state.exists(), "{interaction:?}");
            }
        }
    }

    #[test]
    fn a_connection_appends_one_whole_line_and_reads_back() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new("record");
        let history = scratch.history();
        history.record(&host("lab-a"), at(NOW - 3600)).unwrap();
        history.record(&host("lab-b"), at(NOW)).unwrap();
        let text = fs::read_to_string(scratch.file(FILE)).unwrap();
        assert_eq!(text, format!("{}\tlab-a\n{NOW}\tlab-b\n", NOW - 3600));
        let mode = |path: PathBuf| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(scratch.0.join(DIR)), 0o700);
        assert_eq!(mode(scratch.file(FILE)), 0o600);
        let recents = history.recents(at(NOW));
        assert_eq!(recents.last["lab-a"], NOW - 3600);
        assert_eq!(recents.last["lab-b"], NOW);
    }

    #[test_case(""; "empty")]
    #[test_case("\n\n"; "blank lines")]
    #[test_case("lab-a\n"; "no tab")]
    #[test_case("12x\tlab-a\n"; "a time that is not digits")]
    #[test_case("-5\tlab-a\n"; "a negative time")]
    #[test_case("+5\tlab-a\n"; "a signed time")]
    #[test_case("\tlab-a\n"; "no time")]
    #[test_case("5\t\n"; "no name")]
    #[test_case("99999999999999999999999\tlab-a\n"; "a time that overflows")]
    fn a_line_that_is_not_whole_is_skipped(line: &str) {
        let scratch = Scratch::new("bad-lines");
        let history = scratch.history();
        history.record(&host("good-a"), at(5)).unwrap();
        let mut file = OpenOptions::new()
            .append(true)
            .open(scratch.file(FILE))
            .unwrap();
        file.write_all(line.as_bytes()).unwrap();
        file.write_all(b"\n7\tgood-b\n").unwrap();
        assert_eq!(recent_names(&history), ["good-a", "good-b"], "{line:?}");
    }

    #[test]
    fn a_torn_last_line_is_skipped() {
        let scratch = Scratch::new("torn");
        let history = scratch.history();
        fs::create_dir_all(scratch.0.join(DIR)).unwrap();
        fs::write(scratch.file(FILE), "5\tgood-a\n1794556800\tlab-").unwrap();
        assert_eq!(recent_names(&history), ["good-a"]);
    }

    #[test]
    fn bytes_that_are_not_utf8_cost_only_their_own_line() {
        let scratch = Scratch::new("utf8");
        let history = scratch.history();
        fs::create_dir_all(scratch.0.join(DIR)).unwrap();
        fs::write(
            scratch.file(FILE),
            b"1\tgood-a\n2\tbad\xff\xfe\n3\tgood-b\n",
        )
        .unwrap();
        let names = recent_names(&history);
        assert!(names.contains(&"good-a".to_owned()), "{names:?}");
        assert!(names.contains(&"good-b".to_owned()), "{names:?}");
        assert!(!names.contains(&"bad".to_owned()), "{names:?}");
    }

    #[test]
    fn a_second_tab_leaves_room_for_a_later_field() {
        assert_eq!(
            parse("9\tlab-a\textra\tmore\n").collect::<Vec<_>>(),
            [("lab-a", 9)]
        );
    }

    #[test]
    fn the_latest_time_wins_whatever_the_line_order() {
        let recents = Recents::collect(
            [("a", 5), ("a", 9), ("a", 7), ("b", 1), ("a", 9)].into_iter(),
            NOW,
        );
        assert_eq!((recents.last["a"], recents.last["b"]), (9, 1));
        let same = Recents::collect([("a", 9), ("a", 9)].into_iter(), NOW);
        assert_eq!(same.last["a"], 9);
        let of = Recents::of([("a", at(5)), ("a", at(9)), ("a", at(7))], at(NOW));
        assert_eq!(of.last["a"], 9);
    }

    #[test]
    fn a_time_in_the_future_counts_as_now() {
        let recents = Recents::of([("a", at(NOW + 500))], at(NOW));
        let last = recents.last(&entry("a", &[])).unwrap();
        assert_eq!((last.at, last.ago.as_str()), (NOW, "now"));
    }

    #[test]
    fn a_host_is_last_used_when_any_of_its_names_was() {
        let recents = Recents::of(
            [("web", at(NOW - 7200)), ("web.corp", at(NOW - 60))],
            at(NOW),
        );
        let last = recents.last(&entry("web2", &["web", "web.corp"])).unwrap();
        assert_eq!((last.at, last.ago.as_str()), (NOW - 60, "1m"));
        let only = recents.last(&entry("web", &[])).unwrap();
        assert_eq!(only.ago, "2h");
        assert_eq!(recents.last(&entry("db", &[])), None, "unknown names");
        assert_eq!(Recents::none().last(&entry("web", &[])), None);
    }

    #[test]
    fn a_recorded_name_that_no_host_has_is_ignored() {
        let scratch = Scratch::new("unknown");
        let history = scratch.history();
        history.record(&host("gone"), at(NOW)).unwrap();
        let recents = history.recents(at(NOW));
        assert_eq!(recents.last(&entry("lab-a", &[])), None);
    }

    #[test]
    fn past_the_limit_the_file_becomes_the_old_one_and_both_are_read() {
        let scratch = Scratch::new("rotate");
        let history = scratch.history();
        fs::create_dir_all(scratch.0.join(DIR)).unwrap();
        fs::write(scratch.file(FILE), big("old")).unwrap();
        history.record(&host("new"), at(NOW)).unwrap();
        assert!(!scratch.file(FILE).exists());
        assert!(scratch.file(OLD).exists());
        assert_eq!(recent_names(&history), ["new", "old"]);

        history.record(&host("fresh"), at(NOW)).unwrap();
        assert_eq!(recent_names(&history), ["fresh", "new", "old"]);

        fs::write(scratch.file(FILE), big("mid")).unwrap();
        history.record(&host("last"), at(NOW)).unwrap();
        assert_eq!(
            recent_names(&history),
            ["last", "mid"],
            "the second rotation drops the older generation"
        );
    }

    #[test]
    fn a_short_file_is_not_rotated() {
        let scratch = Scratch::new("short");
        let history = scratch.history();
        for _ in 0..3 {
            history.record(&host("lab-a"), at(NOW)).unwrap();
        }
        assert!(!scratch.file(OLD).exists());
    }

    #[test]
    fn a_bloated_file_is_read_from_its_end() {
        let scratch = Scratch::new("bloated");
        let history = scratch.history();
        fs::create_dir_all(scratch.0.join(DIR)).unwrap();
        let filler = "1\tancient\n".repeat(usize::try_from(READ).unwrap() / 10);
        fs::write(scratch.file(FILE), format!("1\tcut\n{filler}9\trecent\n")).unwrap();
        assert_eq!(recent_names(&history), ["ancient", "recent"]);
    }

    #[test]
    fn a_history_linked_to_dev_null_keeps_nothing_and_never_rotates() {
        let scratch = Scratch::new("dev-null");
        let history = scratch.history();
        fs::create_dir_all(scratch.0.join(DIR)).unwrap();
        std::os::unix::fs::symlink("/dev/null", scratch.file(FILE)).unwrap();
        for _ in 0..3 {
            history.record(&host("lab-a"), at(NOW)).unwrap();
        }
        assert!(history.recents(at(NOW)).last.is_empty());
        assert!(!scratch.file(OLD).exists());
    }

    #[test]
    fn a_save_that_fails_names_the_path_and_a_read_that_fails_is_no_history() {
        let scratch = Scratch::new("errors");
        fs::write(scratch.0.join(DIR), b"in the way").unwrap();
        let history = scratch.history();
        let error = history.record(&host("lab-a"), at(NOW)).unwrap_err();
        let want = format!(
            "cannot save recent hosts to {}: ",
            scratch.0.join(DIR).display()
        );
        assert!(error.to_string().starts_with(&want), "{error}");
        assert!(history.recents(at(NOW)).last.is_empty());

        let scratch = Scratch::new("directory-in-place");
        fs::create_dir_all(scratch.file(FILE)).unwrap();
        let history = scratch.history();
        assert!(history.recents(at(NOW)).last.is_empty());
        assert!(history.record(&host("lab-a"), at(NOW)).is_err());
    }

    #[test]
    fn eight_writers_leave_whole_lines() {
        let scratch = Scratch::new("threads");
        let history = scratch.history();
        thread::scope(|scope| {
            for writer in 0..8 {
                let (history, name) = (&history, host(&format!("writer-{writer}")));
                scope.spawn(move || {
                    for _ in 0..100 {
                        history.record(&name, at(NOW)).unwrap();
                    }
                });
            }
        });
        let text = fs::read_to_string(scratch.file(FILE)).unwrap();
        assert_eq!(parse(&text).count(), 800);
        assert_eq!(text.lines().count(), 800);
    }

    #[test_case(0, "now")]
    #[test_case(59, "now")]
    #[test_case(60, "1m")]
    #[test_case(59 * 60 + 59, "59m")]
    #[test_case(3600, "1h")]
    #[test_case(23 * 3600 + 3599, "23h")]
    #[test_case(86_400, "1d")]
    #[test_case(13 * 86_400 + 86_399, "13d")]
    #[test_case(14 * 86_400, "2w")]
    #[test_case(51 * 7 * 86_400, "51w")]
    #[test_case(364 * 86_400, "52w")]
    #[test_case(365 * 86_400, "1y")]
    #[test_case(12 * 365 * 86_400, "12y")]
    #[test_case(u64::MAX, "99y"; "never wider than three cells")]
    fn an_age_is_at_most_three_cells(seconds: u64, want: &str) {
        assert_eq!(ago(seconds), want);
    }
}
