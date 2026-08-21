use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::ports::{
    Availability, Clock, CommandError, FileError, Files, Lease, Locks, MirrorPlan, Mirrors,
    PathState, ProcessQuery, Processes, Repository, Sighting, Staged,
};
use crate::value::{Branch, CommitId, Timestamp, ValueError};

pub const GAME_PROCESS: &str = "D2R.exe";
pub const GAME_PATTERN: &str = r"(^|[/\\])D2R\.exe([[:space:]]|$)";
pub const MIRROR_DIRECTORY: &str = "saves";
pub const EXCLUDED: [&str; 2] = ["*.ctlo", "*.keyo"];
pub const REQUIRED_COMMANDS: [&str; 3] = ["git", "pgrep", "rsync"];
pub const MAIN_BRANCH: &str = "main";
pub const ORIGIN_REMOTE: &str = "origin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Running,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    LockHeldElsewhere,
    GameRunning,
    GameStartedDuringSync,
    NothingChanged,
    Committed { commit: CommitId, at: Timestamp },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupPaths {
    pub source: PathBuf,
    pub repo: PathBuf,
    pub lock: PathBuf,
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("required command not found: {command}")]
    MissingCommand { command: String },

    #[error("save directory is missing or unsafe: {path}")]
    UnsafeSource { path: PathBuf },

    #[error("backup repository is missing or unsafe: {path}")]
    UnsafeRepository { path: PathBuf },

    #[error("unexpected repository root: {found}")]
    UnexpectedRoot { found: PathBuf },

    #[error("backup repository is not on a branch")]
    DetachedHead,

    #[error("backup repository must be on {expected}, found: {found}")]
    WrongBranch { expected: String, found: Branch },

    #[error("backup repository has no {remote} remote")]
    MissingRemote { remote: String },

    #[error("mirror path is unsafe: {path}")]
    UnsafeMirror { path: PathBuf },

    #[error("unexpected mirror path: {found}")]
    UnexpectedMirror { found: PathBuf },

    #[error("the lock path has no parent directory: {path}")]
    LockWithoutDirectory { path: PathBuf },

    #[error("runtime directory not found: {path}")]
    LockRootMissing { path: PathBuf },

    #[error(transparent)]
    File(#[from] FileError),

    #[error(transparent)]
    Command(#[from] CommandError),

    #[error(transparent)]
    Value(#[from] ValueError),
}

#[derive(Debug)]
pub struct Backup<'a> {
    pub files: &'a dyn Files,
    pub locks: &'a dyn Locks,
    pub processes: &'a dyn Processes,
    pub mirrors: &'a dyn Mirrors,
    pub repository: &'a dyn Repository,
    pub clock: &'a dyn Clock,
}

impl Backup<'_> {
    pub fn run(&self, paths: &BackupPaths) -> Result<SyncOutcome, SyncError> {
        self.required_commands()?;
        self.safe_source(&paths.source)?;
        let root = self.repository_root(&paths.repo)?;
        self.on_main(&paths.repo)?;
        self.has_origin(&paths.repo)?;

        let Lease::Held(_lease) = self.take_lock(&paths.lock)? else {
            return Ok(SyncOutcome::LockHeldElsewhere);
        };
        if self.game()? == GameState::Running {
            return Ok(SyncOutcome::GameRunning);
        }

        let mirror = self.safe_mirror(&root)?;
        self.mirrors.mirror(MirrorPlan {
            source: &paths.source,
            target: &mirror,
            excluded: &EXCLUDED,
        })?;

        if self.game()? == GameState::Running {
            return Ok(SyncOutcome::GameStartedDuringSync);
        }
        self.commit(&paths.repo)
    }

    fn required_commands(&self) -> Result<(), SyncError> {
        for command in REQUIRED_COMMANDS {
            if self.processes.available(command)? == Availability::Missing {
                return Err(SyncError::MissingCommand {
                    command: command.to_owned(),
                });
            }
        }
        Ok(())
    }

    fn safe_source(&self, source: &Path) -> Result<(), SyncError> {
        match self.files.probe(source)? {
            PathState::Directory => Ok(()),
            PathState::Missing | PathState::Symlink | PathState::File => {
                Err(SyncError::UnsafeSource {
                    path: source.to_owned(),
                })
            }
        }
    }

    fn repository_root(&self, repo: &Path) -> Result<PathBuf, SyncError> {
        let unsafe_repository = || SyncError::UnsafeRepository {
            path: repo.to_owned(),
        };
        if self.files.probe(repo)? != PathState::Directory {
            return Err(unsafe_repository());
        }
        if self.files.probe(&repo.join(".git"))? != PathState::Directory {
            return Err(unsafe_repository());
        }
        let root = self.files.real_path(&self.repository.root(repo)?)?;
        if root != self.files.real_path(repo)? {
            return Err(SyncError::UnexpectedRoot { found: root });
        }
        Ok(root)
    }

    fn on_main(&self, repo: &Path) -> Result<(), SyncError> {
        let branch = self
            .repository
            .branch(repo)?
            .ok_or(SyncError::DetachedHead)?;
        if branch.as_str() != MAIN_BRANCH {
            return Err(SyncError::WrongBranch {
                expected: MAIN_BRANCH.to_owned(),
                found: branch,
            });
        }
        Ok(())
    }

    fn has_origin(&self, repo: &Path) -> Result<(), SyncError> {
        if self.repository.remote(repo, ORIGIN_REMOTE)? == Availability::Missing {
            return Err(SyncError::MissingRemote {
                remote: ORIGIN_REMOTE.to_owned(),
            });
        }
        Ok(())
    }

    fn take_lock(&self, lock: &Path) -> Result<Lease, SyncError> {
        let missing = || SyncError::LockWithoutDirectory {
            path: lock.to_owned(),
        };
        let directory = lock.parent().ok_or_else(missing)?;
        let root = directory.parent().ok_or_else(missing)?;
        if self.files.probe(root)? != PathState::Directory {
            return Err(SyncError::LockRootMissing {
                path: root.to_owned(),
            });
        }
        self.files.create_directory(directory)?;
        Ok(self.locks.hold(lock)?)
    }

    fn game(&self) -> Result<GameState, SyncError> {
        Ok(game(self.processes)?)
    }

    fn safe_mirror(&self, root: &Path) -> Result<PathBuf, SyncError> {
        let mirror = root.join(MIRROR_DIRECTORY);
        match self.files.probe(&mirror)? {
            PathState::Symlink | PathState::File => {
                return Err(SyncError::UnsafeMirror { path: mirror });
            }
            PathState::Missing => self.files.create_directory(&mirror)?,
            PathState::Directory => {}
        }
        let real = self.files.real_path(&mirror)?;
        if real != root.join(MIRROR_DIRECTORY) {
            return Err(SyncError::UnexpectedMirror { found: real });
        }
        Ok(real)
    }

    fn commit(&self, repo: &Path) -> Result<SyncOutcome, SyncError> {
        let mirror = Path::new(MIRROR_DIRECTORY);
        self.repository.stage(repo, &[mirror])?;
        if self.repository.staged(repo, &[mirror])? == Staged::Nothing {
            return Ok(SyncOutcome::NothingChanged);
        }
        let at = self.clock.now()?;
        let commit = self
            .repository
            .commit(repo, &format!("backup: {at}"), &[mirror])?;
        Ok(SyncOutcome::Committed { commit, at })
    }
}

pub(crate) fn game(processes: &dyn Processes) -> Result<GameState, CommandError> {
    for query in [
        ProcessQuery::ExactName(GAME_PROCESS),
        ProcessQuery::CommandPattern(GAME_PATTERN),
    ] {
        if processes.look_up(query)? == Sighting::Seen {
            return Ok(GameState::Running);
        }
    }
    Ok(GameState::Closed)
}
