mod support;

use std::path::{Path, PathBuf};

use app::ports::{Files, PathState};
use app::sync::{
    Backup, BackupPaths, EXCLUDED, GAME_PATTERN, GAME_PROCESS, REQUIRED_COMMANDS, SyncError,
    SyncOutcome,
};

use support::{AT, FakeClock, FakeFiles, FakeGit, FakeLocks, FakeMirrors, FakeProcesses};

const SOURCE: &str = "/games/moved";
const REPO: &str = "/backup/repo";
const MIRROR: &str = "/backup/repo/saves";
const RUNTIME: &str = "/run/user/1000";
const LOCK: &str = "/run/user/1000/d2r-backup/lock";

fn paths() -> BackupPaths {
    BackupPaths {
        source: PathBuf::from(SOURCE),
        repo: PathBuf::from(REPO),
        lock: PathBuf::from(LOCK),
    }
}

#[derive(Debug)]
struct Rig {
    files: FakeFiles,
    locks: FakeLocks,
    processes: FakeProcesses,
    mirrors: FakeMirrors,
    git: FakeGit,
    clock: FakeClock,
}

impl Rig {
    fn new() -> Self {
        let files = FakeFiles::new();
        files
            .directory(SOURCE)
            .directory(REPO)
            .directory(format!("{REPO}/.git"))
            .directory(MIRROR)
            .directory(RUNTIME);
        Self {
            files,
            locks: FakeLocks::holding(),
            processes: FakeProcesses::quiet(),
            mirrors: FakeMirrors::new(),
            git: FakeGit::at(REPO),
            clock: FakeClock::default(),
        }
    }

    fn run(&self) -> Result<SyncOutcome, SyncError> {
        Backup {
            files: &self.files,
            locks: &self.locks,
            processes: &self.processes,
            mirrors: &self.mirrors,
            repository: &self.git,
            clock: &self.clock,
        }
        .run(&paths())
    }
}

fn exact() -> String {
    format!("-x {GAME_PROCESS}")
}

fn pattern() -> String {
    format!("-f {GAME_PATTERN}")
}

#[test]
fn every_command_the_backup_spawns_is_required_before_anything_else() {
    for command in REQUIRED_COMMANDS {
        let rig = Rig::new();
        rig.processes.without(command);
        let error = rig.run().expect_err("a missing command stops the backup");
        assert!(
            matches!(&error, SyncError::MissingCommand { command: missing } if missing == command),
            "{command}: {error}"
        );
        assert!(rig.mirrors.runs.borrow().is_empty());
    }
}

#[test]
fn a_missing_save_directory_stops_the_backup() {
    let rig = Rig::new();
    let files = FakeFiles::new();
    files.directory(REPO).directory(format!("{REPO}/.git"));
    let rig = Rig { files, ..rig };
    assert!(matches!(
        rig.run().expect_err("no saves, no backup"),
        SyncError::UnsafeSource { .. }
    ));
}

#[test]
fn a_save_directory_that_is_a_symlink_stops_the_backup() {
    let rig = Rig::new();
    rig.files.symlink(SOURCE);
    assert!(matches!(
        rig.run().expect_err("a symlink is not a safe source"),
        SyncError::UnsafeSource { .. }
    ));
}

#[test]
fn a_repository_without_a_git_directory_stops_the_backup() {
    let files = FakeFiles::new();
    files.directory(SOURCE).directory(REPO);
    let rig = Rig {
        files,
        ..Rig::new()
    };
    assert!(matches!(
        rig.run().expect_err("no .git, no backup"),
        SyncError::UnsafeRepository { .. }
    ));
}

#[test]
fn a_repository_that_is_a_symlink_stops_the_backup() {
    let rig = Rig::new();
    rig.files.symlink(REPO);
    assert!(matches!(
        rig.run().expect_err("a symlink is not a safe repository"),
        SyncError::UnsafeRepository { .. }
    ));
}

#[test]
fn a_repository_root_that_is_not_the_repository_stops_the_backup() {
    let rig = Rig::new();
    rig.files.directory("/work/elsewhere");
    let rig = Rig {
        git: FakeGit::at("/work/elsewhere"),
        ..rig
    };
    assert!(matches!(
        rig.run()
            .expect_err("a nested repository is not the backup repository"),
        SyncError::UnexpectedRoot { .. }
    ));
}

#[test]
fn a_repository_off_any_branch_stops_the_backup() {
    let rig = Rig::new();
    let rig = Rig {
        git: FakeGit::at(REPO).on(None),
        ..rig
    };
    assert!(matches!(
        rig.run().expect_err("a detached head is not a branch"),
        SyncError::DetachedHead
    ));
}

#[test]
fn a_repository_off_main_stops_the_backup() {
    let rig = Rig::new();
    let rig = Rig {
        git: FakeGit::at(REPO).on(Some("feature/other")),
        ..rig
    };
    assert!(matches!(
        rig.run().expect_err("the backup lives on main"),
        SyncError::WrongBranch { .. }
    ));
}

#[test]
fn a_repository_without_an_origin_remote_stops_the_backup() {
    let rig = Rig::new();
    let rig = Rig {
        git: FakeGit::at(REPO).without_origin(),
        ..rig
    };
    assert!(matches!(
        rig.run()
            .expect_err("the backup repository pushes to origin"),
        SyncError::MissingRemote { .. }
    ));
}

#[test]
fn a_lock_another_run_holds_ends_the_backup_without_mirroring() {
    let rig = Rig {
        locks: FakeLocks::busy(),
        ..Rig::new()
    };
    assert_eq!(
        rig.run().expect("a busy lock is not a failure"),
        SyncOutcome::LockHeldElsewhere
    );
    assert!(rig.mirrors.runs.borrow().is_empty());
    assert!(
        rig.processes.asked().is_empty(),
        "the game is checked after the lock"
    );
    assert_eq!(rig.locks.asked.borrow().len(), 1);
}

#[test]
fn the_lock_directory_is_created_under_a_runtime_directory_that_exists() {
    let rig = Rig::new();
    rig.run().expect("the backup runs");
    assert_eq!(
        rig.files
            .probe(Path::new("/run/user/1000/d2r-backup"))
            .expect("probe"),
        PathState::Directory
    );
    assert_eq!(rig.locks.asked.borrow().as_slice(), [PathBuf::from(LOCK)]);
}

#[test]
fn a_missing_runtime_directory_stops_the_backup_instead_of_building_one() {
    let files = FakeFiles::new();
    files
        .directory(SOURCE)
        .directory(REPO)
        .directory(format!("{REPO}/.git"))
        .directory(MIRROR);
    let rig = Rig {
        files,
        ..Rig::new()
    };
    assert!(matches!(
        rig.run().expect_err("a mistyped lock path builds nothing"),
        SyncError::LockRootMissing { .. }
    ));
    assert_eq!(
        rig.files
            .probe(Path::new("/run/user/1000/d2r-backup"))
            .expect("probe"),
        PathState::Missing
    );
    assert!(rig.locks.asked.borrow().is_empty());
}

#[test]
fn a_mirror_that_resolves_outside_the_repository_stops_the_backup() {
    let rig = Rig::new();
    rig.files.directory("/elsewhere/saves");
    rig.files.resolves(MIRROR, "/elsewhere/saves");
    assert!(matches!(
        rig.run()
            .expect_err("the mirror resolves outside the repository"),
        SyncError::UnexpectedMirror { .. }
    ));
    assert!(rig.mirrors.runs.borrow().is_empty());
}

#[test]
fn the_game_seen_by_its_exact_name_ends_the_backup_without_mirroring() {
    let rig = Rig::new();
    rig.processes.sees(&exact());
    assert_eq!(
        rig.run().expect("a running game is not a failure"),
        SyncOutcome::GameRunning
    );
    assert!(rig.mirrors.runs.borrow().is_empty());
}

#[test]
fn the_game_seen_by_its_command_pattern_ends_the_backup_without_mirroring() {
    let rig = Rig::new();
    rig.processes.sees(&pattern());
    assert_eq!(
        rig.run().expect("a running game is not a failure"),
        SyncOutcome::GameRunning
    );
    assert!(rig.mirrors.runs.borrow().is_empty());
    assert_eq!(rig.processes.asked(), vec![exact(), pattern()]);
}

#[test]
fn a_mirror_path_that_is_a_symlink_stops_the_backup() {
    let rig = Rig::new();
    rig.files.symlink(MIRROR);
    assert!(matches!(
        rig.run().expect_err("a symlink is not a safe mirror"),
        SyncError::UnsafeMirror { .. }
    ));
}

#[test]
fn a_mirror_path_that_is_a_file_stops_the_backup() {
    let rig = Rig::new();
    rig.files.file(MIRROR, Vec::new());
    assert!(matches!(
        rig.run().expect_err("a file is not a safe mirror"),
        SyncError::UnsafeMirror { .. }
    ));
}

#[test]
fn a_missing_mirror_directory_is_created_before_the_mirror_runs() {
    let files = FakeFiles::new();
    files
        .directory(SOURCE)
        .directory(REPO)
        .directory(format!("{REPO}/.git"))
        .directory(RUNTIME);
    let rig = Rig {
        files,
        ..Rig::new()
    };
    rig.run().expect("the backup creates its mirror");
    assert_eq!(rig.mirrors.runs.borrow().len(), 1);
    assert_eq!(rig.mirrors.runs.borrow()[0].target, PathBuf::from(MIRROR));
}

#[test]
fn the_mirror_carries_the_flags_and_the_exclusions_of_the_backup_script() {
    let rig = Rig::new();
    rig.run().expect("the backup runs");
    let runs = rig.mirrors.runs.borrow();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].source, PathBuf::from(SOURCE));
    assert_eq!(runs[0].target, PathBuf::from(MIRROR));
    assert_eq!(runs[0].excluded, EXCLUDED);
}

#[test]
fn a_game_that_starts_during_the_mirror_defers_the_commit() {
    let rig = Rig::new();
    rig.processes.sees_later(&exact());
    assert_eq!(
        rig.run().expect("a deferred commit is not a failure"),
        SyncOutcome::GameStartedDuringSync
    );
    assert_eq!(rig.mirrors.runs.borrow().len(), 1);
    assert!(
        !rig.git
            .called()
            .iter()
            .any(|call| call.starts_with("commit"))
    );
}

#[test]
fn a_mirror_that_changed_nothing_leaves_no_commit() {
    let rig = Rig {
        git: FakeGit::at(REPO).clean(),
        ..Rig::new()
    };
    assert_eq!(
        rig.run().expect("the backup runs"),
        SyncOutcome::NothingChanged
    );
    assert_eq!(
        rig.git.called(),
        ["root", "branch", "remote", "stage saves", "staged"]
    );
}

#[test]
fn a_mirror_that_changed_something_commits_the_timestamp() {
    let rig = Rig::new();
    let outcome = rig.run().expect("the backup runs");
    let SyncOutcome::Committed { at, .. } = &outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(at.as_str(), AT);
    assert_eq!(
        rig.git.messages.borrow().as_slice(),
        [format!("backup: {AT}")]
    );
    assert_eq!(
        rig.git.called(),
        [
            "root",
            "branch",
            "remote",
            "stage saves",
            "staged",
            "commit saves"
        ]
    );
}
