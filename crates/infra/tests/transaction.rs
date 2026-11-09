mod support;

use std::path::{Path, PathBuf};

use app::ledger::{LEDGER_DIRECTORY, LEDGER_FILE, LedgerEntry};
use app::ports::{TimerState, Timers};
use app::transaction::{
    Applied, PlanDocument, Record, Transaction, TransactionError, TransactionPaths,
};
use app::value::UnitName;
use app::world::SaveTree;
use domain::census::Census;
use domain::container::{GridId, PileId, StashName, TabIndex};
use domain::object::Fingerprint;
use domain::vault::Vault;
use format::save::CharacterSave;
use format::stash::StashSave;
use format::stats::PlayerStats;
use infra::files::RealFiles;
use infra::git::GitCommands;
use infra::system::SystemdTimers;

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

fn gold(saves: &Path) -> Vec<(String, u32)> {
    let bytes = std::fs::read(saves.join(format!("{STASH}.d2i"))).expect("the stash");
    StashSave::read(&bytes)
        .expect("the stash reads")
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| (format!("tab {index} {:?}", tab.kind()), tab.gold))
        .collect()
}

fn character_stats(saves: &Path) -> Vec<(String, PlayerStats)> {
    let mut held: Vec<(String, PlayerStats)> = Vec::new();
    for name in ["Gous", "Elektra"] {
        let bytes = std::fs::read(saves.join(format!("{name}.d2s"))).expect("the character");
        let save = CharacterSave::read(&bytes).expect("the character reads");
        held.push((name.to_owned(), save.stats));
    }
    held
}

fn hoard(saves: &Path, tab: usize, amount: u32) {
    let path = saves.join(format!("{STASH}.d2i"));
    let mut save = StashSave::read(&std::fs::read(&path).expect("the stash")).expect("the stash");
    save.tabs[tab].gold = amount;
    std::fs::write(&path, save.write().expect("the stash writes")).expect("the stash");
}

#[test]
fn no_operation_moves_a_single_piece_of_gold() {
    let tree = TempTree::new("gold");
    let repo = repository(&tree, "backup");
    let saves = corpus(&repo);
    let rig = Rig::new();

    hoard(&saves, 5, 4_242);
    git(&repo, &["add", "-A", "--", "saves"]);
    git(&repo, &["commit", "-m", "gold in the advanced stash tab"]);

    let before = gold(&saves);
    let stats = character_stats(&saves);
    assert!(
        before.iter().any(|(_, amount)| *amount == 1_075_411),
        "{before:?}"
    );
    assert_eq!(
        before
            .iter()
            .find(|(name, _)| name.contains("AdvancedStash"))
            .map(|(_, amount)| *amount),
        Some(4_242),
        "{before:?}"
    );

    let fingerprint = first(&vault(&saves), 0);
    rig.apply(
        &repo,
        &saves,
        &plan(&format!(
            "move:shared/{STASH}/0#{fingerprint}:shared/{STASH}/2"
        )),
    )
    .expect("the move applies");
    assert_eq!(gold(&saves), before, "a move moved gold");
    assert_eq!(
        character_stats(&saves),
        stats,
        "a move changed a character stat"
    );

    let carried = first(&vault(&saves), 1);
    rig.apply(
        &repo,
        &saves,
        &plan(&format!(
            "duplicate:shared/{STASH}/1#{carried}:char/Elektra/cube"
        )),
    )
    .expect("the duplication applies");
    assert_eq!(gold(&saves), before, "a duplication moved gold");
    assert_eq!(
        character_stats(&saves),
        stats,
        "a duplication changed a character stat"
    );

    let entry = vault(&saves)
        .pile(&pile(5))
        .expect("the advanced stash tab")
        .entries()
        .first()
        .expect("the tab holds units")
        .clone();
    rig.apply(
        &repo,
        &saves,
        &plan(&format!(
            "move:shared/{STASH}/5#{}*{}:shared/{STASH}/5",
            entry.object().fingerprint(),
            entry.count().get()
        )),
    )
    .expect("the pile move applies");
    assert_eq!(gold(&saves), before, "a pile move moved gold");
    assert_eq!(
        character_stats(&saves),
        stats,
        "a pile move changed a character stat"
    );
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
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
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
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
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
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
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
fn a_repository_dirty_before_the_apply_gets_its_own_return_point() {
    let tree = TempTree::new("checkpoint");
    let repo = repository(&tree, "backup");
    let saves = corpus(&repo);
    let rig = Rig::new();
    std::fs::write(saves.join("note.txt"), b"left behind by the game\n").expect("stray file");

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

    assert_eq!(
        git(
            &repo,
            &["log", "-1", "--format=%s", applied.checkpoint.as_str()]
        ),
        format!("checkpoint: {AT}")
    );
    assert_eq!(
        git(&repo, &["rev-parse", "HEAD~1"]),
        applied.checkpoint.as_str()
    );
    assert_eq!(
        git(
            &repo,
            &[
                "show",
                "--name-only",
                "--format=",
                applied.checkpoint.as_str()
            ]
        ),
        "saves/note.txt"
    );
}
