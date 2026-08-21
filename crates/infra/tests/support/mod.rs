#![allow(dead_code, reason = "each test binary drives a subset of this harness")]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use app::ports::{
    Availability, Clock, CommandError, ProcessQuery, Processes, Sighting, TimerState, Timers,
};
use app::value::{Timestamp, UnitName, ValueError};

pub(crate) const AT: &str = "2026-08-21T02:13:19+02:00";

static COUNTER: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
pub(crate) struct TempTree {
    root: PathBuf,
}

impl TempTree {
    pub(crate) fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "d2r-manager-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("temporary tree");
        Self { root }
    }

    #[must_use]
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn directory(&self, relative: &str) -> PathBuf {
        let path = self.root.join(relative);
        std::fs::create_dir_all(&path).expect("directory");
        path
    }

    pub(crate) fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub(crate) fn git(repo: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {}: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned()
}

pub(crate) fn repository(tree: &TempTree, name: &str) -> PathBuf {
    let repo = tree.directory(name);
    let origin = tree.directory(&format!("{name}-origin.git"));
    Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg("-b")
        .arg("main")
        .arg(&origin)
        .output()
        .expect("git init --bare");
    Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .arg(&repo)
        .output()
        .expect("git init");
    git(&repo, &["config", "user.email", "tests@example.invalid"]);
    git(&repo, &["config", "user.name", "d2r-manager tests"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    git(
        &repo,
        &["remote", "add", "origin", &origin.to_string_lossy()],
    );
    std::fs::write(repo.join("README.md"), "backup repository\n").expect("seed file");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-m", "init"]);
    repo
}

pub(crate) fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .canonicalize()
        .expect("fixtures")
}

pub(crate) fn copy_fixture(relative: &str, target: &Path) {
    std::fs::copy(fixture_root().join(relative), target).expect("fixture copy");
}

#[derive(Debug, Default)]
pub(crate) struct StillProcesses {
    seen: RefCell<BTreeSet<String>>,
    pub(crate) asked: RefCell<Vec<String>>,
}

impl StillProcesses {
    #[must_use]
    pub(crate) fn quiet() -> Self {
        Self::default()
    }

    pub(crate) fn sees(&self, query: &str) -> &Self {
        self.seen.borrow_mut().insert(query.to_owned());
        self
    }
}

impl Processes for StillProcesses {
    fn look_up(&self, query: ProcessQuery<'_>) -> Result<Sighting, CommandError> {
        let key = match query {
            ProcessQuery::ExactName(name) => format!("-x {name}"),
            ProcessQuery::CommandPattern(pattern) => format!("-f {pattern}"),
        };
        self.asked.borrow_mut().push(key.clone());
        Ok(if self.seen.borrow().contains(&key) {
            Sighting::Seen
        } else {
            Sighting::Absent
        })
    }

    fn available(&self, _command: &str) -> Result<Availability, CommandError> {
        Ok(Availability::Present)
    }
}

#[derive(Debug)]
pub(crate) struct RecordedTimers {
    pub(crate) state: TimerState,
    pub(crate) calls: RefCell<Vec<String>>,
}

impl RecordedTimers {
    #[must_use]
    pub(crate) fn active() -> Self {
        Self {
            state: TimerState::Active,
            calls: RefCell::new(Vec::new()),
        }
    }

    #[must_use]
    pub(crate) fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl Timers for RecordedTimers {
    fn state(&self, _unit: &UnitName) -> Result<TimerState, CommandError> {
        self.calls.borrow_mut().push("state".to_owned());
        Ok(self.state)
    }

    fn pause(&self, _unit: &UnitName) -> Result<(), CommandError> {
        self.calls.borrow_mut().push("pause".to_owned());
        Ok(())
    }

    fn resume(&self, _unit: &UnitName) -> Result<(), CommandError> {
        self.calls.borrow_mut().push("resume".to_owned());
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SteadyClock {
    pub(crate) at: String,
}

impl Default for SteadyClock {
    fn default() -> Self {
        Self { at: AT.to_owned() }
    }
}

impl Clock for SteadyClock {
    fn now(&self) -> Result<Timestamp, ValueError> {
        Timestamp::new(&self.at)
    }
}
