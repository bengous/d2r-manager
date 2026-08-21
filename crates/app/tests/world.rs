mod support;

use std::path::Path;

use app::transaction::{
    CellDocument, DestinationDocument, OperationDocument, PlanDocument, SourceDocument,
    TransferDocument, resolve,
};
use app::world::{SaveTree, WorldError};
use domain::census::Census;
use domain::container::{GridId, PileId, StashName, TabIndex};
use domain::object::Fingerprint;
use domain::vault::Vault;

use support::{FakeFiles, fixture, fixture_names};

const SAVES: &str = "/saves";
const LINUX: &str = "v105/linux";
const STASH: &str = "linux";

fn corpus() -> FakeFiles {
    let files = FakeFiles::new();
    files.directory(SAVES);
    for name in fixture_names(LINUX) {
        files.file(
            Path::new(SAVES).join(&name),
            fixture(&format!("{LINUX}/{name}")),
        );
    }
    files
}

fn one_stash() -> FakeFiles {
    let files = FakeFiles::new();
    files.directory(SAVES);
    files.file(
        Path::new(SAVES).join(format!("{STASH}.d2i")),
        fixture(&format!("{LINUX}/ModernSharedStashSoftCoreV2.d2i")),
    );
    files
}

fn stash() -> StashName {
    StashName::new(STASH).expect("stash name")
}

fn tab(index: u8) -> GridId {
    GridId::SharedTab {
        stash: stash(),
        tab: TabIndex::new(index),
    }
}

fn pile(index: u8) -> PileId {
    PileId::new(stash(), TabIndex::new(index))
}

fn first(vault: &Vault, index: u8) -> Fingerprint {
    vault
        .grid(&tab(index))
        .expect("tab")
        .slots()
        .first()
        .expect("the tab holds an item")
        .object()
        .fingerprint()
}

fn moved(from: u8, fingerprint: Fingerprint, to: &str, cell: Option<CellDocument>) -> PlanDocument {
    PlanDocument {
        operations: vec![OperationDocument {
            transfer: TransferDocument::Move,
            from: SourceDocument {
                container: format!("shared/{STASH}/{from}"),
                fingerprint: fingerprint.to_string(),
                units: None,
            },
            to: DestinationDocument {
                container: to.to_owned(),
                cell,
            },
        }],
    }
}

#[test]
fn the_whole_corpus_writes_back_byte_for_byte_when_the_world_does_not_change() {
    let files = corpus();
    let mut tree = SaveTree::read(&files, Path::new(SAVES)).expect("the saves read");
    let vault = tree.vault().expect("the vault builds");
    tree.project(&vault).expect("the world projects back");
    let written = tree.flush(&files).expect("the saves write");
    assert!(
        written.is_empty(),
        "an unchanged world rewrote {} file(s): {written:?}",
        written.len()
    );
}

#[test]
fn every_character_exposes_its_three_pages_even_when_they_hold_nothing() {
    let files = corpus();
    let tree = SaveTree::read(&files, Path::new(SAVES)).expect("the saves read");
    let vault = tree.vault().expect("the vault builds");
    let mut characters = 0;
    for name in fixture_names(LINUX) {
        let Some(stem) = name.strip_suffix(".d2s") else {
            continue;
        };
        characters += 1;
        for page in app::world::CHARACTER_PAGES {
            let id = GridId::Character {
                character: domain::container::CharacterName::new(stem).expect("character name"),
                page,
            };
            assert!(vault.grid(&id).is_some(), "{stem}: {page} is absent");
        }
    }
    assert_eq!(characters, 53);
}

#[test]
fn a_move_between_two_tabs_reads_back_as_the_plan_expected() {
    let files = one_stash();
    let mut tree = SaveTree::read(&files, Path::new(SAVES)).expect("the saves read");
    let vault = tree.vault().expect("the vault builds");
    let fingerprint = first(&vault, 0);
    let footprint = vault
        .grid(&tab(0))
        .expect("tab 0")
        .find(fingerprint)
        .expect("the item")
        .footprint();
    let free = vault
        .grid(&tab(1))
        .expect("tab 1")
        .first_free(footprint)
        .expect("tab 1 has room");
    let document = moved(
        0,
        fingerprint,
        &format!("shared/{STASH}/1"),
        Some(CellDocument {
            column: free.column().get(),
            row: free.row().get(),
        }),
    );

    let plan = resolve(&document, &vault).expect("the plan resolves");
    let validated = plan.validate(vault).expect("the plan is valid");
    tree.project(validated.outcome())
        .expect("the world projects");
    let written = tree.flush(&files).expect("the saves write");
    assert_eq!(written.len(), 1);

    let observed = SaveTree::read(&files, Path::new(SAVES))
        .expect("the saves read back")
        .vault()
        .expect("the vault builds again");
    let applied = validated
        .apply(&observed)
        .expect("the saves read back as the plan expected");
    assert_eq!(applied.realized().len(), 1);
    assert!(applied.duplicated().is_empty());

    assert!(
        observed
            .grid(&tab(0))
            .expect("tab 0")
            .find(fingerprint)
            .is_none()
    );
    assert!(
        observed
            .grid(&tab(1))
            .expect("tab 1")
            .find(fingerprint)
            .is_some()
    );
}

#[test]
fn an_item_without_a_stack_field_never_enters_an_advanced_stash_tab() {
    let files = one_stash();
    let mut tree = SaveTree::read(&files, Path::new(SAVES)).expect("the saves read");
    let vault = tree.vault().expect("the vault builds");
    let fingerprint = first(&vault, 0);
    let document = moved(0, fingerprint, &format!("shared/{STASH}/5"), None);

    let plan = resolve(&document, &vault).expect("the plan resolves");
    let validated = plan.validate(vault).expect("the plan is valid");
    let refusal = tree
        .project(validated.outcome())
        .expect_err("a grid item carries no unit count");
    assert!(
        matches!(&refusal, WorldError::StackAbsent { fingerprint: found, .. } if *found == fingerprint),
        "{refusal}"
    );
    assert!(
        tree.flush(&files).expect("the saves write").is_empty(),
        "a refused projection writes nothing"
    );
}

#[test]
fn a_move_inside_the_advanced_stash_tab_keeps_every_unit() {
    let files = one_stash();
    let mut tree = SaveTree::read(&files, Path::new(SAVES)).expect("the saves read");
    let vault = tree.vault().expect("the vault builds");
    let entry = vault
        .pile(&pile(5))
        .expect("the advanced stash tab")
        .entries()
        .first()
        .expect("the tab holds units")
        .clone();
    let before = Census::of(&vault).totals();

    let document = PlanDocument {
        operations: vec![OperationDocument {
            transfer: TransferDocument::Move,
            from: SourceDocument {
                container: format!("shared/{STASH}/5"),
                fingerprint: entry.object().fingerprint().to_string(),
                units: Some(entry.count().get()),
            },
            to: DestinationDocument {
                container: format!("shared/{STASH}/5"),
                cell: None,
            },
        }],
    };
    let plan = resolve(&document, &vault).expect("the plan resolves");
    let validated = plan.validate(vault).expect("the plan is valid");
    tree.project(validated.outcome())
        .expect("the world projects");
    tree.flush(&files).expect("the saves write");

    let observed = SaveTree::read(&files, Path::new(SAVES))
        .expect("the saves read back")
        .vault()
        .expect("the vault builds again");
    validated
        .apply(&observed)
        .expect("the saves read back as the plan expected");
    assert_eq!(Census::of(&observed).totals(), before);
}
