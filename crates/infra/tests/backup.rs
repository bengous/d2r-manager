mod support;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use app::ports::Clock;
use app::ports::{Availability, Lease, Locks, MirrorPlan, Processes, Repository};
use app::sync::{Backup, BackupPaths, EXCLUDED, GAME_PROCESS, SyncOutcome};
use infra::files::{FileLock, RealFiles};
use infra::git::{GitCommands, stage_arguments};
use infra::system::{Rsync, SystemClock, SystemProcesses, rsync_arguments};

use support::{AT, SteadyClock, StillProcesses, TempTree, copy_fixture, git, repository};

fn seeded(tree: &TempTree) -> PathBuf {
    let source = tree.directory("saves");
    copy_fixture(
        "v105/linux/ModernSharedStashSoftCoreV2.d2i",
        &source.join("linux.d2i"),
    );
    copy_fixture("v105/linux/Gous.d2s", &source.join("Gous.d2s"));
    std::fs::write(source.join("Gous.ctlo"), b"controller layout").expect("excluded file");
    std::fs::write(source.join("Gous.keyo"), b"key layout").expect("excluded file");
    source
}

fn run(tree: &TempTree, source: &Path, repo: &Path, processes: &StillProcesses) -> SyncOutcome {
    let files = RealFiles;
    let locks = FileLock;
    let mirrors = Rsync;
    let repository = GitCommands;
    let clock = SteadyClock::default();
    Backup {
        files: &files,
        locks: &locks,
        processes,
        mirrors: &mirrors,
        repository: &repository,
        clock: &clock,
    }
    .run(&BackupPaths {
        source: source.to_owned(),
        repo: repo.to_owned(),
        lock: tree.path("lock/backup.lock"),
    })
    .expect("the backup runs")
}

#[test]
fn the_rsync_arguments_carry_the_flags_of_the_backup_script() {
    let arguments = rsync_arguments(MirrorPlan {
        source: Path::new("/games/moved"),
        target: Path::new("/backup/repo/saves"),
        excluded: &EXCLUDED,
    });
    assert_eq!(
        arguments,
        vec![
            OsString::from("--archive"),
            OsString::from("--delete"),
            OsString::from("--delete-excluded"),
            OsString::from("--exclude=*.ctlo"),
            OsString::from("--exclude=*.keyo"),
            OsString::from("--"),
            OsString::from("/games/moved/"),
            OsString::from("/backup/repo/saves/"),
        ]
    );
}

#[test]
fn the_mirror_holds_the_saves_and_drops_the_layouts() {
    let tree = TempTree::new("mirror");
    let source = seeded(&tree);
    let repo = repository(&tree, "backup");

    let outcome = run(&tree, &source, &repo, &StillProcesses::quiet());
    let SyncOutcome::Committed { at, .. } = &outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(at.as_str(), AT);
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%s"]),
        format!("backup: {AT}")
    );

    let mirror = repo.join("saves");
    assert!(mirror.join("linux.d2i").is_file());
    assert!(mirror.join("Gous.d2s").is_file());
    assert!(!mirror.join("Gous.ctlo").exists());
    assert!(!mirror.join("Gous.keyo").exists());
    assert_eq!(
        std::fs::read(mirror.join("linux.d2i")).expect("mirrored stash"),
        std::fs::read(source.join("linux.d2i")).expect("source stash")
    );
}

#[test]
fn a_second_run_over_the_same_saves_commits_nothing() {
    let tree = TempTree::new("idle");
    let source = seeded(&tree);
    let repo = repository(&tree, "backup");

    run(&tree, &source, &repo, &StillProcesses::quiet());
    let before = git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(
        run(&tree, &source, &repo, &StillProcesses::quiet()),
        SyncOutcome::NothingChanged
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), before);
}

#[test]
fn a_save_the_mirror_no_longer_holds_leaves_the_mirror() {
    let tree = TempTree::new("delete");
    let source = seeded(&tree);
    let repo = repository(&tree, "backup");

    run(&tree, &source, &repo, &StillProcesses::quiet());
    std::fs::remove_file(source.join("Gous.d2s")).expect("the save leaves");
    run(&tree, &source, &repo, &StillProcesses::quiet());
    assert!(!repo.join("saves/Gous.d2s").exists());
    assert!(repo.join("saves/linux.d2i").is_file());
}

#[test]
fn a_lock_another_handle_holds_stops_the_mirror() {
    let tree = TempTree::new("lock");
    let source = seeded(&tree);
    let repo = repository(&tree, "backup");
    let lock = tree.path("lock/backup.lock");
    std::fs::create_dir_all(lock.parent().expect("lock directory")).expect("lock directory");

    let held = FileLock;
    let Lease::Held(lease) = held.hold(&lock).expect("the first handle locks") else {
        panic!("the first handle takes the lock");
    };

    let outcome = run(&tree, &source, &repo, &StillProcesses::quiet());
    assert_eq!(outcome, SyncOutcome::LockHeldElsewhere);
    assert!(!repo.join("saves").exists());

    drop(lease);
    let outcome = run(&tree, &source, &repo, &StillProcesses::quiet());
    assert!(
        matches!(outcome, SyncOutcome::Committed { .. }),
        "a released lock leaves the mirror free: {outcome:?}"
    );
}

#[test]
fn a_running_game_stops_the_mirror() {
    let tree = TempTree::new("running");
    let source = seeded(&tree);
    let repo = repository(&tree, "backup");
    let processes = StillProcesses::quiet();
    processes.sees(&format!("-x {GAME_PROCESS}"));

    assert_eq!(
        run(&tree, &source, &repo, &processes),
        SyncOutcome::GameRunning
    );
    assert!(!repo.join("saves").exists());
}

#[test]
fn every_command_the_adapters_spawn_is_installed_on_this_machine() {
    let processes = SystemProcesses;
    for command in app::sync::REQUIRED_COMMANDS {
        assert_eq!(
            processes.available(command).expect("the lookup runs"),
            Availability::Present,
            "{command}"
        );
    }
    assert_eq!(
        processes
            .available("d2r-manager-no-such-command")
            .expect("the lookup runs"),
        Availability::Missing
    );
}

#[test]
fn the_clock_prints_the_stamp_the_backup_script_writes() {
    let stamp = SystemClock.now().expect("the clock reads").to_string();
    let bytes = stamp.as_bytes();
    assert_eq!(bytes.len(), 25, "{stamp}");
    assert_eq!(bytes[4], b'-');
    assert_eq!(bytes[7], b'-');
    assert_eq!(bytes[10], b'T');
    assert_eq!(bytes[13], b':');
    assert_eq!(bytes[16], b':');
    assert!(bytes[19] == b'+' || bytes[19] == b'-', "{stamp}");
    assert_eq!(bytes[22], b':');
    assert!(
        bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| !matches!(index, 0..=3 | 5..=6 | 8..=9 | 11..=12 | 14..=15 | 17..=18 | 20..=21 | 23..=24)
                || byte.is_ascii_digit()),
        "{stamp}"
    );
}

#[test]
fn staging_takes_the_new_and_the_deleted_inside_the_pathspec_and_nothing_outside() {
    let tree = TempTree::new("stage");
    let repo = repository(&tree, "backup");
    let saves = repo.join("saves");
    std::fs::create_dir_all(&saves).expect("saves directory");
    std::fs::write(saves.join("kept.d2s"), b"kept").expect("seed");
    std::fs::write(saves.join("gone.d2s"), b"gone").expect("seed");
    git(&repo, &["add", "-A", "--", "saves"]);
    git(&repo, &["commit", "-m", "seed"]);

    std::fs::write(saves.join("fresh.d2s"), b"fresh").expect("a save the game added");
    std::fs::remove_file(saves.join("gone.d2s")).expect("a save the game removed");
    std::fs::write(repo.join("outside.txt"), b"not a save").expect("a file outside the mirror");

    GitCommands
        .stage(&repo, &[Path::new("saves")])
        .expect("the staging runs");

    let mut staged: Vec<String> = git(&repo, &["diff", "--cached", "--name-status"])
        .lines()
        .map(str::to_owned)
        .collect();
    staged.sort();
    assert_eq!(staged, ["A\tsaves/fresh.d2s", "D\tsaves/gone.d2s"]);
    assert_eq!(
        git(&repo, &["status", "--porcelain", "--", "outside.txt"]),
        "?? outside.txt"
    );
}

#[test]
fn the_staging_arguments_carry_the_flags_of_the_backup_script() {
    assert_eq!(
        stage_arguments(&[Path::new("saves")]),
        vec![
            OsString::from("add"),
            OsString::from("-A"),
            OsString::from("--"),
            OsString::from("saves"),
        ]
    );
}
