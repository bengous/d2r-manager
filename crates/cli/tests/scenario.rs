mod support;

use std::path::{Path, PathBuf};

use clap::Parser;

use app::ledger::{LEDGER_DIRECTORY, LEDGER_FILE, LedgerEntry};
use app::ports::{TimerState, Timers};
use app::transaction::{
    Applied, PlanDocument, Record, Transaction, TransactionError, TransactionPaths,
};
use app::value::UnitName;
use app::world::SaveTree;
use cli::adapters::files::RealFiles;
use cli::adapters::git::GitCommands;
use cli::adapters::system::SystemdTimers;
use cli::options::Options;
use domain::census::Census;
use domain::container::{GridId, PileId, StashName, TabIndex};
use domain::object::Fingerprint;
use domain::vault::Vault;

use support::{
    AT, RecordedTimers, SteadyClock, StillProcesses, TempTree, copy_fixture, git, repository,
};

const STASH: &str = "linux";
const ABSENT_TIMER: &str = "d2r-manager-tests-no-such.timer";

fn corpus(repo: &Path) -> PathBuf {
    let saves = repo.join("saves");
    std::fs::create_dir_all(&saves).expect("saves directory");
    copy_fixture(
        "v105/linux/ModernSharedStashSoftCoreV2.d2i",
        &saves.join(format!("{STASH}.d2i")),
    );
    copy_fixture("v105/linux/Gous.d2s", &saves.join("Gous.d2s"));
    copy_fixture("v105/linux/Elektra.d2s", &saves.join("Elektra.d2s"));
    git(repo, &["add", "-A", "--", "saves"]);
    git(repo, &["commit", "-m", "saves"]);
    saves
}

fn vault(saves: &Path) -> Vault {
    SaveTree::read(&RealFiles, saves)
        .expect("the saves read")
        .vault()
        .expect("the vault builds")
}

fn tab(index: u8) -> GridId {
    GridId::SharedTab {
        stash: StashName::new(STASH).expect("stash"),
        tab: TabIndex::new(index),
    }
}

fn pile(index: u8) -> PileId {
    PileId::new(StashName::new(STASH).expect("stash"), TabIndex::new(index))
}

fn first(vault: &Vault, index: u8) -> Fingerprint {
    vault
        .grid(&tab(index))
        .expect("the tab")
        .slots()
        .first()
        .expect("the tab holds an item")
        .object()
        .fingerprint()
}

fn plan(text: &str) -> PlanDocument {
    PlanDocument {
        operations: vec![app::shorthand::parse(text).expect("the shorthand reads")],
    }
}

#[derive(Debug)]
struct Rig {
    processes: StillProcesses,
    timers: RecordedTimers,
    clock: SteadyClock,
}

impl Rig {
    fn new() -> Self {
        Self {
            processes: StillProcesses::quiet(),
            timers: RecordedTimers::active(),
            clock: SteadyClock::default(),
        }
    }

    fn apply(
        &self,
        repo: &Path,
        saves: &Path,
        document: &PlanDocument,
    ) -> Result<Applied, TransactionError> {
        Transaction {
            files: &RealFiles,
            processes: &self.processes,
            repository: &GitCommands,
            timers: &self.timers,
            clock: &self.clock,
        }
        .apply(
            &TransactionPaths {
                repo: repo.to_owned(),
                saves: saves.to_owned(),
                timer: UnitName::new("d2r-backup.timer").expect("unit"),
            },
            document,
        )
    }
}

#[test]
fn a_move_applies_reads_back_and_keeps_every_item() {
    let tree = TempTree::new("apply");
    let repo = repository(&tree, "backup");
    let saves = corpus(&repo);
    let rig = Rig::new();

    let before = vault(&saves);
    let held = Census::of(&before).totals();
    let fingerprint = first(&before, 0);

    let applied = rig
        .apply(
            &repo,
            &saves,
            &plan(&format!(
                "move:shared/{STASH}/0#{fingerprint}:shared/{STASH}/2"
            )),
        )
        .expect("the plan applies");

    let Record::Committed { commit, at } = &applied.record else {
        panic!("{:?}", applied.record);
    };
    assert_eq!(at.as_str(), AT);
    assert_eq!(rig.timers.called(), ["state", "pause", "resume"]);

    let after = vault(&saves);
    assert_eq!(Census::of(&after).totals(), held);
    assert!(
        after
            .grid(&tab(0))
            .expect("tab 0")
            .find(fingerprint)
            .is_none()
    );
    assert!(
        after
            .grid(&tab(2))
            .expect("tab 2")
            .find(fingerprint)
            .is_some()
    );

    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), commit.as_str());
    let message = git(&repo, &["log", "-1", "--format=%B"]);
    assert!(message.starts_with(&format!("apply: {AT}")), "{message}");
    assert!(message.contains("\"transfer\": \"move\""), "{message}");
    assert_eq!(
        git(&repo, &["show", "--name-only", "--format=", "HEAD"]),
        format!("saves/{STASH}.d2i")
    );
    assert!(git(&repo, &["status", "--porcelain"]).is_empty());
}

#[test]
fn a_revert_of_the_apply_commit_puts_every_byte_back() {
    let tree = TempTree::new("revert");
    let repo = repository(&tree, "backup");
    let saves = corpus(&repo);
    let rig = Rig::new();

    let stash = saves.join(format!("{STASH}.d2i"));
    let bytes = std::fs::read(&stash).expect("the stash");
    let before = Census::of(&vault(&saves)).totals();
    let fingerprint = first(&vault(&saves), 0);

    let applied = rig
        .apply(
            &repo,
            &saves,
            &plan(&format!(
                "move:shared/{STASH}/0#{fingerprint}:shared/{STASH}/2"
            )),
        )
        .expect("the plan applies");
    let Record::Committed { commit, .. } = &applied.record else {
        panic!("{:?}", applied.record);
    };
    assert_ne!(std::fs::read(&stash).expect("the stash"), bytes);

    git(&repo, &["revert", "--no-edit", commit.as_str()]);

    assert_eq!(std::fs::read(&stash).expect("the stash"), bytes);
    let back = vault(&saves);
    assert_eq!(Census::of(&back).totals(), before);
    assert!(
        back.grid(&tab(0))
            .expect("tab 0")
            .find(fingerprint)
            .is_some()
    );
    assert!(
        back.grid(&tab(2))
            .expect("tab 2")
            .find(fingerprint)
            .is_none()
    );
    assert!(git(&repo, &["status", "--porcelain"]).is_empty());
}

#[test]
fn a_duplication_writes_the_ledger_inside_the_commit_of_the_operation() {
    let tree = TempTree::new("ledger");
    let repo = repository(&tree, "backup");
    let saves = corpus(&repo);
    let rig = Rig::new();

    let before = vault(&saves);
    let fingerprint = first(&before, 0);
    assert!(!repo.join(LEDGER_DIRECTORY).exists());

    let applied = rig
        .apply(
            &repo,
            &saves,
            &plan(&format!(
                "duplicate:shared/{STASH}/0#{fingerprint}:char/Elektra/cube"
            )),
        )
        .expect("the plan applies");

    let ledger = repo.join(LEDGER_DIRECTORY).join(LEDGER_FILE);
    let text = std::fs::read_to_string(&ledger).expect("the ledger exists");
    let entry: LedgerEntry = serde_json::from_str(text.trim_end()).expect("the line reads back");
    assert_eq!(entry.fingerprint, fingerprint.to_string());
    assert_eq!(entry.from, format!("shared/{STASH}/0"));
    assert_eq!(entry.to, "char/Elektra/cube");
    assert_eq!(entry.units, 1);
    assert_eq!(entry.at, AT);
    assert_eq!(applied.duplications, vec![entry]);

    let mut names: Vec<String> = git(&repo, &["show", "--name-only", "--format=", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            format!("{LEDGER_DIRECTORY}/{LEDGER_FILE}"),
            "saves/Elektra.d2s".to_owned(),
        ]
    );
    assert_eq!(applied.written, vec![saves.join("Elektra.d2s")]);

    let after = vault(&saves);
    assert_eq!(
        Census::of(&after).totals()[&fingerprint],
        Census::of(&before).totals()[&fingerprint] + 1
    );
    assert!(git(&repo, &["status", "--porcelain"]).is_empty());
}

#[test]
fn a_second_duplication_appends_a_line_to_the_ledger_it_already_holds() {
    let tree = TempTree::new("ledger-twice");
    let repo = repository(&tree, "backup");
    let saves = corpus(&repo);
    let rig = Rig::new();

    let fingerprint = first(&vault(&saves), 0);
    for target in ["char/Elektra/cube", "char/Gous/cube"] {
        rig.apply(
            &repo,
            &saves,
            &plan(&format!(
                "duplicate:shared/{STASH}/0#{fingerprint}:{target}"
            )),
        )
        .expect("the plan applies");
    }

    let text = std::fs::read_to_string(repo.join(LEDGER_DIRECTORY).join(LEDGER_FILE))
        .expect("the ledger exists");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    let targets: Vec<String> = lines
        .iter()
        .map(|line| {
            serde_json::from_str::<LedgerEntry>(line)
                .expect("the line reads back")
                .to
        })
        .collect();
    assert_eq!(targets, ["char/Elektra/cube", "char/Gous/cube"]);
}

#[test]
fn a_move_of_units_inside_the_advanced_stash_tab_keeps_the_count() {
    let tree = TempTree::new("units");
    let repo = repository(&tree, "backup");
    let saves = corpus(&repo);
    let rig = Rig::new();

    let before = vault(&saves);
    let entry = before
        .pile(&pile(5))
        .expect("the advanced stash tab")
        .entries()
        .first()
        .expect("the tab holds units")
        .clone();
    let held = Census::of(&before).totals();

    rig.apply(
        &repo,
        &saves,
        &plan(&format!(
            "move:shared/{STASH}/5#{}*{}:shared/{STASH}/5",
            entry.object().fingerprint(),
            entry.count().get()
        )),
    )
    .expect("the plan applies");

    let after = vault(&saves);
    assert_eq!(Census::of(&after).totals(), held);
}

#[test]
fn the_ledger_lives_outside_the_directory_rsync_mirrors() {
    let tree = TempTree::new("outside");
    let repo = repository(&tree, "backup");
    let saves = corpus(&repo);
    let rig = Rig::new();

    let fingerprint = first(&vault(&saves), 0);
    rig.apply(
        &repo,
        &saves,
        &plan(&format!(
            "duplicate:shared/{STASH}/0#{fingerprint}:char/Elektra/cube"
        )),
    )
    .expect("the plan applies");

    let ledger = repo.join(LEDGER_DIRECTORY).join(LEDGER_FILE);
    assert!(ledger.is_file());
    assert!(!ledger.starts_with(&saves), "{}", ledger.display());
    assert_eq!(
        ledger.parent().expect("directory"),
        repo.join(LEDGER_DIRECTORY)
    );
}

#[test]
fn a_unit_no_timer_declares_reads_back_as_absent() {
    let unit = UnitName::new(ABSENT_TIMER).expect("unit");
    assert_eq!(
        SystemdTimers.state(&unit).expect("systemctl answers"),
        TimerState::Absent
    );
}

#[test]
fn the_binary_applies_a_plan_file_end_to_end() {
    let tree = TempTree::new("binary");
    let repo = repository(&tree, "backup");
    let saves = corpus(&repo);

    let fingerprint = first(&vault(&saves), 0);
    let file = tree.path("plan.json");
    std::fs::write(
        &file,
        serde_json::to_string_pretty(&plan(&format!(
            "move:shared/{STASH}/0#{fingerprint}:shared/{STASH}/2"
        )))
        .expect("the plan serializes"),
    )
    .expect("the plan file");

    let options = Options::try_parse_from([
        "d2r-manager",
        "apply",
        "--repo",
        &repo.to_string_lossy(),
        "--plan",
        &file.to_string_lossy(),
        "--timer",
        ABSENT_TIMER,
    ])
    .expect("the arguments parse");
    let mut out = Vec::new();
    cli::run(&options, &mut out).expect("the apply runs; Diablo II Resurrected must be closed");
    let text = String::from_utf8(out).expect("utf-8");

    assert!(text.contains("apply: committed"), "{text}");
    assert!(text.contains(&fingerprint.to_string()), "{text}");
    let after = vault(&saves);
    assert!(
        after
            .grid(&tab(2))
            .expect("tab 2")
            .find(fingerprint)
            .is_some()
    );
    assert!(
        git(&repo, &["log", "-1", "--format=%s"]).starts_with("apply: "),
        "the binary committed nothing"
    );
}
