#![allow(dead_code, reason = "each test binary drives a subset of these fakes")]
#![allow(
    clippy::disallowed_methods,
    reason = "PLAN 6.5: the fakes seed themselves from the frozen corpus"
)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use app::ports::{
    Availability, Clock, CommandError, FileError, Files, Guard, Lease, Locks, MirrorPlan, Mirrors,
    PathState, ProcessQuery, Processes, Repository, Sighting, Staged, TimerState, Timers,
};
use app::value::{Branch, CommitId, Timestamp, UnitName, ValueError};

pub(crate) const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
pub(crate) const NEXT: &str = "89abcdef0123456789abcdef0123456789abcdef";
pub(crate) const AT: &str = "2026-08-21T02:13:19+02:00";

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

pub(crate) fn fixture(relative: &str) -> Vec<u8> {
    let path = repo_root().join("fixtures").join(relative);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

pub(crate) fn fixture_names(directory: &str) -> Vec<String> {
    let path = repo_root().join("fixtures").join(directory);
    let mut names: Vec<String> = std::fs::read_dir(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .map(|entry| entry.expect("directory entry").file_name())
        .filter_map(|name| name.into_string().ok())
        .filter(|name| {
            matches!(
                Path::new(name).extension().and_then(|part| part.to_str()),
                Some("d2s" | "d2i")
            )
        })
        .collect();
    names.sort();
    names
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entry {
    Directory,
    Symlink,
    File(Vec<u8>),
}

#[derive(Debug, Default)]
pub(crate) struct FakeFiles {
    entries: RefCell<BTreeMap<PathBuf, Entry>>,
    ignored: RefCell<BTreeSet<PathBuf>>,
    resolved: RefCell<BTreeMap<PathBuf, PathBuf>>,
}

impl FakeFiles {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn directory(&self, path: impl AsRef<Path>) -> &Self {
        self.put(path.as_ref(), Entry::Directory);
        self
    }

    pub(crate) fn symlink(&self, path: impl AsRef<Path>) -> &Self {
        self.put(path.as_ref(), Entry::Symlink);
        self
    }

    pub(crate) fn file(&self, path: impl AsRef<Path>, bytes: Vec<u8>) -> &Self {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            self.put(parent, Entry::Directory);
        }
        self.put(path, Entry::File(bytes));
        self
    }

    pub(crate) fn ignore_writes(&self, path: impl AsRef<Path>) -> &Self {
        self.ignored.borrow_mut().insert(path.as_ref().to_owned());
        self
    }

    pub(crate) fn resolves(&self, path: impl AsRef<Path>, target: impl AsRef<Path>) -> &Self {
        self.resolved
            .borrow_mut()
            .insert(path.as_ref().to_owned(), target.as_ref().to_owned());
        self
    }

    #[must_use]
    pub(crate) fn bytes(&self, path: impl AsRef<Path>) -> Option<Vec<u8>> {
        match self.entries.borrow().get(path.as_ref()) {
            Some(Entry::File(bytes)) => Some(bytes.clone()),
            _ => None,
        }
    }

    #[must_use]
    pub(crate) fn text(&self, path: impl AsRef<Path>) -> Option<String> {
        self.bytes(path)
            .map(|bytes| String::from_utf8(bytes).expect("utf-8"))
    }

    fn put(&self, path: &Path, entry: Entry) {
        let mut entries = self.entries.borrow_mut();
        for parent in path.ancestors().skip(1) {
            entries.entry(parent.to_owned()).or_insert(Entry::Directory);
        }
        entries.insert(path.to_owned(), entry);
    }
}

impl Files for FakeFiles {
    fn probe(&self, path: &Path) -> Result<PathState, FileError> {
        Ok(match self.entries.borrow().get(path) {
            None => PathState::Missing,
            Some(Entry::Directory) => PathState::Directory,
            Some(Entry::Symlink) => PathState::Symlink,
            Some(Entry::File(_)) => PathState::File,
        })
    }

    fn real_path(&self, path: &Path) -> Result<PathBuf, FileError> {
        if let Some(target) = self.resolved.borrow().get(path) {
            return Ok(target.clone());
        }
        if self.entries.borrow().contains_key(path) {
            return Ok(path.to_owned());
        }
        Err(missing(path, "resolve"))
    }

    fn create_directory(&self, path: &Path) -> Result<(), FileError> {
        self.put(path, Entry::Directory);
        Ok(())
    }

    fn entries(&self, path: &Path) -> Result<Vec<PathBuf>, FileError> {
        let entries = self.entries.borrow();
        if !matches!(entries.get(path), Some(Entry::Directory)) {
            return Err(missing(path, "list"));
        }
        Ok(entries
            .keys()
            .filter(|candidate| candidate.parent() == Some(path))
            .cloned()
            .collect())
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
        match self.entries.borrow().get(path) {
            Some(Entry::File(bytes)) => Ok(bytes.clone()),
            _ => Err(missing(path, "read")),
        }
    }

    fn write(&self, path: &Path, data: &[u8]) -> Result<(), FileError> {
        if self.ignored.borrow().contains(path) {
            return Ok(());
        }
        self.put(path, Entry::File(data.to_vec()));
        Ok(())
    }

    fn append_line(&self, path: &Path, line: &str) -> Result<(), FileError> {
        let mut bytes = self.bytes(path).unwrap_or_default();
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
        self.put(path, Entry::File(bytes));
        Ok(())
    }
}

fn missing(path: &Path, action: &'static str) -> FileError {
    FileError {
        path: path.to_owned(),
        action,
        source: std::io::Error::from(ErrorKind::NotFound),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grant {
    Held,
    Busy,
}

#[derive(Debug)]
pub(crate) struct FakeLocks {
    pub(crate) grant: Grant,
    pub(crate) asked: RefCell<Vec<PathBuf>>,
}

impl FakeLocks {
    #[must_use]
    pub(crate) fn holding() -> Self {
        Self {
            grant: Grant::Held,
            asked: RefCell::new(Vec::new()),
        }
    }

    #[must_use]
    pub(crate) fn busy() -> Self {
        Self {
            grant: Grant::Busy,
            asked: RefCell::new(Vec::new()),
        }
    }
}

impl Locks for FakeLocks {
    fn hold(&self, path: &Path) -> Result<Lease, FileError> {
        self.asked.borrow_mut().push(path.to_owned());
        Ok(match self.grant {
            Grant::Held => Lease::Held(Box::new(FakeGuard)),
            Grant::Busy => Lease::Busy,
        })
    }
}

#[derive(Debug)]
pub(crate) struct FakeGuard;

impl Guard for FakeGuard {}

#[derive(Debug, Default)]
pub(crate) struct FakeProcesses {
    seen: RefCell<BTreeSet<String>>,
    later: RefCell<BTreeSet<String>>,
    missing: RefCell<BTreeSet<String>>,
    pub(crate) asked: RefCell<Vec<String>>,
}

impl FakeProcesses {
    #[must_use]
    pub(crate) fn quiet() -> Self {
        Self::default()
    }

    pub(crate) fn sees(&self, query: &str) -> &Self {
        self.seen.borrow_mut().insert(query.to_owned());
        self
    }

    pub(crate) fn sees_later(&self, query: &str) -> &Self {
        self.later.borrow_mut().insert(query.to_owned());
        self
    }

    pub(crate) fn without(&self, command: &str) -> &Self {
        self.missing.borrow_mut().insert(command.to_owned());
        self
    }

    #[must_use]
    pub(crate) fn asked(&self) -> Vec<String> {
        self.asked.borrow().clone()
    }
}

#[must_use]
pub(crate) fn describe(query: ProcessQuery<'_>) -> String {
    match query {
        ProcessQuery::ExactName(name) => format!("-x {name}"),
        ProcessQuery::CommandPattern(pattern) => format!("-f {pattern}"),
    }
}

impl Processes for FakeProcesses {
    fn look_up(&self, query: ProcessQuery<'_>) -> Result<Sighting, CommandError> {
        let key = describe(query);
        let before = self
            .asked
            .borrow()
            .iter()
            .filter(|ask| **ask == key)
            .count();
        self.asked.borrow_mut().push(key.clone());
        let seen =
            self.seen.borrow().contains(&key) || (before > 0 && self.later.borrow().contains(&key));
        Ok(if seen {
            Sighting::Seen
        } else {
            Sighting::Absent
        })
    }

    fn available(&self, command: &str) -> Result<Availability, CommandError> {
        Ok(if self.missing.borrow().contains(command) {
            Availability::Missing
        } else {
            Availability::Present
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mirrored {
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
    pub(crate) excluded: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct FakeMirrors {
    pub(crate) runs: RefCell<Vec<Mirrored>>,
}

impl FakeMirrors {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl Mirrors for FakeMirrors {
    fn mirror(&self, plan: MirrorPlan<'_>) -> Result<(), CommandError> {
        self.runs.borrow_mut().push(Mirrored {
            source: plan.source.to_owned(),
            target: plan.target.to_owned(),
            excluded: plan
                .excluded
                .iter()
                .map(|text| (*text).to_owned())
                .collect(),
        });
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct FakeGit {
    pub(crate) root: PathBuf,
    pub(crate) branch: Option<Branch>,
    pub(crate) remote: Availability,
    answers: RefCell<VecDeque<Staged>>,
    otherwise: Staged,
    pub(crate) head: RefCell<CommitId>,
    pub(crate) calls: RefCell<Vec<String>>,
    pub(crate) messages: RefCell<Vec<String>>,
}

impl FakeGit {
    #[must_use]
    pub(crate) fn at(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_owned(),
            branch: Some(Branch::new("main").expect("branch")),
            remote: Availability::Present,
            answers: RefCell::new(VecDeque::new()),
            otherwise: Staged::Changes,
            head: RefCell::new(CommitId::new(HEAD).expect("commit")),
            calls: RefCell::new(Vec::new()),
            messages: RefCell::new(Vec::new()),
        }
    }

    #[must_use]
    pub(crate) fn answering(self, answers: &[Staged]) -> Self {
        *self.answers.borrow_mut() = answers.iter().copied().collect();
        self
    }

    #[must_use]
    pub(crate) fn on(mut self, branch: Option<&str>) -> Self {
        self.branch = branch.map(|name| Branch::new(name).expect("branch"));
        self
    }

    #[must_use]
    pub(crate) fn without_origin(mut self) -> Self {
        self.remote = Availability::Missing;
        self
    }

    #[must_use]
    pub(crate) fn clean(mut self) -> Self {
        self.otherwise = Staged::Nothing;
        self
    }

    #[must_use]
    pub(crate) fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    fn note(&self, call: &str) {
        self.calls.borrow_mut().push(call.to_owned());
    }

    fn note_paths(&self, call: &str, paths: &[&Path]) {
        let listed: Vec<String> = paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        self.calls
            .borrow_mut()
            .push(format!("{call} {}", listed.join(",")));
    }
}

impl Repository for FakeGit {
    fn root(&self, _repo: &Path) -> Result<PathBuf, CommandError> {
        self.note("root");
        Ok(self.root.clone())
    }

    fn branch(&self, _repo: &Path) -> Result<Option<Branch>, CommandError> {
        self.note("branch");
        Ok(self.branch.clone())
    }

    fn remote(&self, _repo: &Path, _name: &str) -> Result<Availability, CommandError> {
        self.note("remote");
        Ok(self.remote)
    }

    fn head(&self, _repo: &Path) -> Result<CommitId, CommandError> {
        self.note("head");
        Ok(self.head.borrow().clone())
    }

    fn stage(&self, _repo: &Path, paths: &[&Path]) -> Result<(), CommandError> {
        self.note_paths("stage", paths);
        Ok(())
    }

    fn staged(&self, _repo: &Path, _paths: &[&Path]) -> Result<Staged, CommandError> {
        self.note("staged");
        Ok(self
            .answers
            .borrow_mut()
            .pop_front()
            .unwrap_or(self.otherwise))
    }

    fn commit(
        &self,
        _repo: &Path,
        message: &str,
        paths: &[&Path],
    ) -> Result<CommitId, CommandError> {
        self.note_paths("commit", paths);
        self.messages.borrow_mut().push(message.to_owned());
        let commit = CommitId::new(NEXT).expect("commit");
        *self.head.borrow_mut() = commit.clone();
        Ok(commit)
    }

    fn restore(
        &self,
        _repo: &Path,
        commit: &CommitId,
        paths: &[&Path],
    ) -> Result<(), CommandError> {
        self.note_paths(&format!("restore {commit}"), paths);
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct FakeTimers {
    pub(crate) state: TimerState,
    pub(crate) calls: RefCell<Vec<String>>,
}

impl FakeTimers {
    #[must_use]
    pub(crate) fn active() -> Self {
        Self {
            state: TimerState::Active,
            calls: RefCell::new(Vec::new()),
        }
    }

    #[must_use]
    pub(crate) fn absent() -> Self {
        Self {
            state: TimerState::Absent,
            calls: RefCell::new(Vec::new()),
        }
    }

    #[must_use]
    pub(crate) fn inactive() -> Self {
        Self {
            state: TimerState::Inactive,
            calls: RefCell::new(Vec::new()),
        }
    }

    #[must_use]
    pub(crate) fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl Timers for FakeTimers {
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
pub(crate) struct FakeClock {
    pub(crate) at: String,
}

impl Default for FakeClock {
    fn default() -> Self {
        Self { at: AT.to_owned() }
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Result<Timestamp, ValueError> {
        Timestamp::new(&self.at)
    }
}
