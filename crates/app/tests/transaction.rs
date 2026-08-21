mod support;

use std::path::{Path, PathBuf};

use app::ledger::{LEDGER_DIRECTORY, LEDGER_FILE, LedgerEntry};
use app::ports::Staged;
use app::sync::GAME_PROCESS;
use app::transaction::{
    CellDocument, DestinationDocument, OperationDocument, PlanDocument, Record, SourceDocument,
    Transaction, TransactionError, TransactionPaths, TransferDocument,
};
use app::value::UnitName;
use app::world::SaveTree;
use domain::census::Census;
use domain::container::{GridId, StashName, TabIndex};
use domain::object::Fingerprint;
use domain::plan::Transfer;

use support::{AT, FakeClock, FakeFiles, FakeGit, FakeProcesses, FakeTimers, fixture};

const REPO: &str = "/backup/repo";
const SAVES: &str = "/backup/repo/saves";
const STASH: &str = "linux";
const TIMER: &str = "d2r-backup.timer";

fn stash_path() -> PathBuf {
    Path::new(SAVES).join(format!("{STASH}.d2i"))
}

fn ledger_path() -> PathBuf {
    Path::new(REPO).join(LEDGER_DIRECTORY).join(LEDGER_FILE)
}

fn world() -> FakeFiles {
    let files = FakeFiles::new();
    files
        .directory(REPO)
        .directory(format!("{REPO}/.git"))
        .directory(SAVES);
    files.file(
        stash_path(),
        fixture("v105/linux/ModernSharedStashSoftCoreV2.d2i"),
    );
    files
}

fn paths() -> TransactionPaths {
    TransactionPaths {
        repo: PathBuf::from(REPO),
        saves: PathBuf::from(SAVES),
        timer: UnitName::new(TIMER).expect("unit"),
    }
}

fn tab(index: u8) -> GridId {
    GridId::SharedTab {
        stash: StashName::new(STASH).expect("stash"),
        tab: TabIndex::new(index),
    }
}

fn first(files: &FakeFiles, index: u8) -> Fingerprint {
    SaveTree::read(files, Path::new(SAVES))
        .expect("the saves read")
        .vault()
        .expect("the vault builds")
        .grid(&tab(index))
        .expect("the tab")
        .slots()
        .first()
        .expect("the tab holds an item")
        .object()
        .fingerprint()
}

fn one(transfer: TransferDocument, fingerprint: Fingerprint, to: u8) -> PlanDocument {
    PlanDocument {
        operations: vec![OperationDocument {
            transfer,
            from: SourceDocument {
                container: format!("shared/{STASH}/0"),
                fingerprint: fingerprint.to_string(),
                units: None,
            },
            to: DestinationDocument {
                container: format!("shared/{STASH}/{to}"),
                cell: None,
            },
        }],
    }
}

#[derive(Debug)]
struct Rig {
    files: FakeFiles,
    processes: FakeProcesses,
    git: FakeGit,
    timers: FakeTimers,
    clock: FakeClock,
}

impl Rig {
    fn new() -> Self {
        Self {
            files: world(),
            processes: FakeProcesses::quiet(),
            git: FakeGit::at(REPO).answering(&[Staged::Nothing, Staged::Changes]),
            timers: FakeTimers::active(),
            clock: FakeClock::default(),
        }
    }

    fn apply(
        &self,
        document: &PlanDocument,
    ) -> Result<app::transaction::Applied, TransactionError> {
        Transaction {
            files: &self.files,
            processes: &self.processes,
            repository: &self.git,
            timers: &self.timers,
            clock: &self.clock,
        }
        .apply(&paths(), document)
    }
}

#[test]
fn a_running_game_stops_the_apply_before_anything_moves() {
    let rig = Rig::new();
    rig.processes.sees(&format!("-x {GAME_PROCESS}"));
    let bytes = rig.files.bytes(stash_path()).expect("the stash");
    let fingerprint = first(&rig.files, 0);

    let error = rig
        .apply(&one(TransferDocument::Move, fingerprint, 1))
        .expect_err("the game holds the saves");
    assert!(matches!(error, TransactionError::GameRunning), "{error}");
    assert_eq!(rig.files.bytes(stash_path()), Some(bytes));
    assert!(rig.timers.called().is_empty());
    assert!(rig.git.called().is_empty());
}

#[test]
fn the_apply_pauses_the_timer_writes_then_resumes_it() {
    let rig = Rig::new();
    let fingerprint = first(&rig.files, 0);
    let applied = rig
        .apply(&one(TransferDocument::Move, fingerprint, 1))
        .expect("the plan applies");

    assert_eq!(rig.timers.called(), ["state", "pause", "resume"]);
    assert_eq!(applied.written, vec![stash_path()]);
    let Record::Committed { at, .. } = &applied.record else {
        panic!("{:?}", applied.record);
    };
    assert_eq!(at.as_str(), AT);
}

#[test]
fn a_timer_no_unit_declares_is_neither_paused_nor_resumed() {
    let rig = Rig {
        timers: FakeTimers::absent(),
        ..Rig::new()
    };
    let fingerprint = first(&rig.files, 0);
    rig.apply(&one(TransferDocument::Move, fingerprint, 1))
        .expect("the plan applies");
    assert_eq!(rig.timers.called(), ["state"]);
}

#[test]
fn the_saves_read_back_hold_the_world_the_plan_expected() {
    let rig = Rig::new();
    let fingerprint = first(&rig.files, 0);
    let before = Census::of(
        &SaveTree::read(&rig.files, Path::new(SAVES))
            .expect("the saves read")
            .vault()
            .expect("the vault builds"),
    )
    .totals();

    let applied = rig
        .apply(&one(TransferDocument::Move, fingerprint, 1))
        .expect("the plan applies");
    assert_eq!(applied.realized.len(), 1);
    assert_eq!(applied.realized[0].transfer(), Transfer::Move);

    let after = SaveTree::read(&rig.files, Path::new(SAVES))
        .expect("the saves read back")
        .vault()
        .expect("the vault builds again");
    assert_eq!(Census::of(&after).totals(), before);
    assert!(
        after
            .grid(&tab(0))
            .expect("tab 0")
            .find(fingerprint)
            .is_none()
    );
    assert!(
        after
            .grid(&tab(1))
            .expect("tab 1")
            .find(fingerprint)
            .is_some()
    );
}

#[test]
fn the_return_point_is_taken_before_the_first_byte_is_written() {
    let rig = Rig::new();
    let fingerprint = first(&rig.files, 0);
    let applied = rig
        .apply(&one(TransferDocument::Move, fingerprint, 1))
        .expect("the plan applies");

    let calls = rig.git.called();
    assert_eq!(calls[0], "root");
    assert_eq!(calls[1], "stage saves");
    assert_eq!(calls[2], "staged");
    assert_eq!(calls[3], "head");
    assert_eq!(applied.checkpoint.as_str(), support::HEAD);
}

#[test]
fn a_duplication_lands_in_the_ledger_and_in_the_commit_of_the_operation() {
    let rig = Rig::new();
    let fingerprint = first(&rig.files, 0);
    let applied = rig
        .apply(&one(TransferDocument::Duplicate, fingerprint, 1))
        .expect("the plan applies");

    assert_eq!(applied.duplications.len(), 1);
    let entry = &applied.duplications[0];
    assert_eq!(
        entry,
        &LedgerEntry {
            at: AT.to_owned(),
            fingerprint: fingerprint.to_string(),
            from: format!("shared/{STASH}/0"),
            to: format!("shared/{STASH}/1"),
            units: 1,
        }
    );

    let text = rig.files.text(ledger_path()).expect("the ledger exists");
    assert_eq!(text.lines().count(), 1);
    let written: LedgerEntry = serde_json::from_str(text.trim_end()).expect("the line reads back");
    assert_eq!(&written, entry);

    let calls = rig.git.called();
    let staged = calls
        .iter()
        .rposition(|call| call.starts_with("stage "))
        .expect("the apply stages");
    assert_eq!(calls[staged], format!("stage saves,{LEDGER_DIRECTORY}"));
    assert_eq!(
        calls.last().expect("the apply commits"),
        &format!("commit saves,{LEDGER_DIRECTORY}")
    );
    let message = rig
        .git
        .messages
        .borrow()
        .last()
        .cloned()
        .expect("a message");
    assert!(message.starts_with(&format!("apply: {AT}")), "{message}");
    assert!(message.contains("\"transfer\": \"duplicate\""), "{message}");

    let after = SaveTree::read(&rig.files, Path::new(SAVES))
        .expect("the saves read back")
        .vault()
        .expect("the vault builds again");
    assert!(
        after
            .grid(&tab(0))
            .expect("tab 0")
            .find(fingerprint)
            .is_some()
    );
    assert!(
        after
            .grid(&tab(1))
            .expect("tab 1")
            .find(fingerprint)
            .is_some()
    );
}

#[test]
fn a_ledger_the_apply_never_writes_stays_out_of_the_commit() {
    let rig = Rig::new();
    let fingerprint = first(&rig.files, 0);
    rig.apply(&one(TransferDocument::Move, fingerprint, 1))
        .expect("the plan applies");

    assert!(rig.files.text(ledger_path()).is_none());
    assert!(
        rig.git
            .called()
            .iter()
            .all(|call| !call.contains(LEDGER_DIRECTORY))
    );
}

#[test]
fn a_write_the_disk_refuses_rolls_the_saves_back_to_the_return_point() {
    let rig = Rig::new();
    rig.files.ignore_writes(stash_path());
    let fingerprint = first(&rig.files, 0);

    let error = rig
        .apply(&one(TransferDocument::Move, fingerprint, 1))
        .expect_err("the saves never took the write");
    assert!(matches!(error, TransactionError::ReadBack(_)), "{error}");

    let calls = rig.git.called();
    assert_eq!(
        calls.last().expect("the apply rolls back"),
        &format!("restore {} saves", support::HEAD)
    );
    assert!(!calls.iter().any(|call| call.starts_with("commit saves,")));
    assert_eq!(rig.timers.called(), ["state", "pause", "resume"]);
}

#[test]
fn a_plan_that_changes_nothing_leaves_no_commit() {
    let rig = Rig {
        git: FakeGit::at(REPO).answering(&[Staged::Nothing, Staged::Nothing]),
        ..Rig::new()
    };
    let applied = rig
        .apply(&PlanDocument {
            operations: Vec::new(),
        })
        .expect("an empty plan applies");
    assert_eq!(applied.record, Record::NothingChanged);
    assert!(applied.written.is_empty());
    assert!(applied.realized.is_empty());
    assert!(
        !rig.git
            .called()
            .iter()
            .any(|call| call.starts_with("commit"))
    );
}

#[test]
fn a_cell_the_grid_refuses_stops_the_plan_before_the_timer_moves() {
    let rig = Rig::new();
    let fingerprint = first(&rig.files, 0);
    let document = PlanDocument {
        operations: vec![OperationDocument {
            transfer: TransferDocument::Move,
            from: SourceDocument {
                container: format!("shared/{STASH}/0"),
                fingerprint: fingerprint.to_string(),
                units: None,
            },
            to: DestinationDocument {
                container: format!("shared/{STASH}/1"),
                cell: Some(CellDocument { column: 0, row: 0 }),
            },
        }],
    };
    let error = rig.apply(&document).expect_err("the cell is taken");
    assert!(matches!(error, TransactionError::Refused(_)), "{error}");
    assert!(rig.timers.called().is_empty());
    assert!(
        !rig.git
            .called()
            .iter()
            .any(|call| call.starts_with("stage"))
    );
}

#[test]
fn saves_outside_the_repository_stop_the_apply() {
    let rig = Rig::new();
    rig.files.directory("/elsewhere/saves");
    let fingerprint = first(&rig.files, 0);
    let outcome = Transaction {
        files: &rig.files,
        processes: &rig.processes,
        repository: &rig.git,
        timers: &rig.timers,
        clock: &rig.clock,
    }
    .apply(
        &TransactionPaths {
            saves: PathBuf::from("/elsewhere/saves"),
            ..paths()
        },
        &one(TransferDocument::Move, fingerprint, 1),
    );
    assert!(
        matches!(
            outcome.expect_err("the saves live outside the repository"),
            TransactionError::SavesOutsideRepository { .. }
        ),
        "the guard let a foreign directory through"
    );
}
