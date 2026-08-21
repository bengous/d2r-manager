use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::value::{Branch, CommitId, Timestamp, UnitName, ValueError};

#[derive(Debug, Error)]
#[error("{path}: {action}: {source}")]
pub struct FileError {
    pub path: PathBuf,
    pub action: &'static str,
    #[source]
    pub source: io::Error,
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("{command}: {source}")]
    Spawn {
        command: String,
        #[source]
        source: io::Error,
    },

    #[error("{command}: exit status {status}: {stderr}")]
    Failed {
        command: String,
        status: String,
        stderr: String,
    },

    #[error("{command}: unexpected output: {output:?}")]
    Output { command: String, output: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathState {
    Missing,
    Symlink,
    Directory,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sighting {
    Seen,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Present,
    Missing,
}

pub trait Guard: fmt::Debug {}

#[derive(Debug)]
pub enum Lease {
    Held(Box<dyn Guard>),
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staged {
    Nothing,
    Changes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    Active,
    Inactive,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessQuery<'a> {
    ExactName(&'a str),
    CommandPattern(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MirrorPlan<'a> {
    pub source: &'a Path,
    pub target: &'a Path,
    pub excluded: &'a [&'a str],
}

pub trait Files: fmt::Debug {
    fn probe(&self, path: &Path) -> Result<PathState, FileError>;
    fn real_path(&self, path: &Path) -> Result<PathBuf, FileError>;
    fn create_directory(&self, path: &Path) -> Result<(), FileError>;
    fn entries(&self, path: &Path) -> Result<Vec<PathBuf>, FileError>;
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError>;
    fn write(&self, path: &Path, data: &[u8]) -> Result<(), FileError>;
    fn append_line(&self, path: &Path, line: &str) -> Result<(), FileError>;
}

pub trait Locks: fmt::Debug {
    fn hold(&self, path: &Path) -> Result<Lease, FileError>;
}

pub trait Processes: fmt::Debug {
    fn look_up(&self, query: ProcessQuery<'_>) -> Result<Sighting, CommandError>;
    fn available(&self, command: &str) -> Result<Availability, CommandError>;
}

pub trait Mirrors: fmt::Debug {
    fn mirror(&self, plan: MirrorPlan<'_>) -> Result<(), CommandError>;
}

pub trait Repository: fmt::Debug {
    fn root(&self, repo: &Path) -> Result<PathBuf, CommandError>;
    fn branch(&self, repo: &Path) -> Result<Option<Branch>, CommandError>;
    fn remote(&self, repo: &Path, name: &str) -> Result<Availability, CommandError>;
    fn head(&self, repo: &Path) -> Result<CommitId, CommandError>;
    fn stage(&self, repo: &Path, paths: &[&Path]) -> Result<(), CommandError>;
    fn staged(&self, repo: &Path, paths: &[&Path]) -> Result<Staged, CommandError>;
    fn commit(&self, repo: &Path, message: &str, paths: &[&Path])
    -> Result<CommitId, CommandError>;
    fn restore(&self, repo: &Path, commit: &CommitId, paths: &[&Path]) -> Result<(), CommandError>;
}

pub trait Timers: fmt::Debug {
    fn state(&self, unit: &UnitName) -> Result<TimerState, CommandError>;
    fn pause(&self, unit: &UnitName) -> Result<(), CommandError>;
    fn resume(&self, unit: &UnitName) -> Result<(), CommandError>;
}

pub trait Clock: fmt::Debug {
    fn now(&self) -> Result<Timestamp, ValueError>;
}
