mod support;

use domain::census::Census;
use domain::container::{ContainerId, GridId, PileId, StashName, TabIndex};
use domain::error::{ApplyError, ModelError, RuleViolation, ValueError, VaultError};
use domain::gold::Gold;
use domain::grid::{Column, GridPosition, Row};
use domain::object::Object;
use domain::pile::{PileCount, PileUnits};
use domain::plan::{Destination, Operation, Plan, Source};
use domain::vault::Vault;

use format::bits::Bits;
use format::item::{
    BodyLocation, GridCell, Item, ItemBody, ItemFlags, ItemPosition, SlotMode, StorePage,
};
use format::stash::{StashSave, TabBody};

use support::{compact, compact_marked, fixture_bytes, stash_name};

const MARKER: &str = "v105/linux/ModernSharedStashSoftCoreV2.d2i";

fn stash() -> StashName {
    stash_name("shared")
}

fn tab(index: u8) -> GridId {
    GridId::SharedTab {
        stash: stash(),
        tab: TabIndex::new(index),
    }
}

fn pile_tab(index: u8) -> PileId {
    PileId::new(stash(), TabIndex::new(index))
}

fn at(column: u8, row: u8) -> GridPosition {
    GridPosition::new(Column::new(column), Row::new(row)).expect("grid position")
}

fn object(item: &Item) -> Object {
    Object::new(item.clone()).expect("object")
}

fn marker() -> StashSave {
    StashSave::read(&fixture_bytes(MARKER)).expect("shared stash")
}

fn first_normal(save: &mut StashSave) -> (usize, &mut Vec<Item>) {
    let index = save
        .tabs
        .iter()
        .position(|tab| matches!(tab.body, TabBody::Normal(_)))
        .expect("a normal tab");
    match &mut save.tabs[index].body {
        TabBody::Normal(items) => (index, &mut items.0),
        TabBody::AdvancedStash(_) | TabBody::Chronicle(_) => unreachable!(),
    }
}

fn build(save: &StashSave) -> Result<Vault, VaultError> {
    Ok(Vault::builder().shared_stash(&stash(), save)?.build())
}

#[test]
fn a_stash_item_outside_the_stored_mode_has_no_container() {
    let mut save = marker();
    let (index, items) = first_normal(&mut save);
    items[0].position = ItemPosition::Slot {
        mode: SlotMode::Equipped,
        body: BodyLocation::Neck,
        cell: GridCell::new(0, 0).expect("cell"),
        page: StorePage::Stash,
    };
    let index = u8::try_from(index).expect("tab index");
    assert_eq!(
        build(&save),
        Err(VaultError::NotStored {
            container: ContainerId::Grid(tab(index)),
        })
    );
}

#[test]
fn a_stash_file_whose_items_overlap_is_refused() {
    let mut save = marker();
    let (index, items) = first_normal(&mut save);
    let cell = match items[0].position {
        ItemPosition::Slot { cell, .. } => cell,
        ItemPosition::Ground { .. } => unreachable!("stash items are stored"),
    };
    if let ItemPosition::Slot { cell: target, .. } = &mut items[1].position {
        *target = cell;
    }
    let index = u8::try_from(index).expect("tab index");
    assert_eq!(
        build(&save),
        Err(VaultError::Placement(RuleViolation::CellTaken {
            container: tab(index),
            cell: GridPosition::from_cell(cell),
        }))
    );
}

#[test]
fn a_tab_over_the_gold_cap_is_refused() {
    let mut save = marker();
    save.tabs[0].gold = Gold::CAP + 1;
    assert_eq!(
        build(&save),
        Err(VaultError::Gold {
            container: ContainerId::Grid(tab(0)),
            source: ValueError::GoldOverCap {
                amount: Gold::CAP + 1,
                cap: Gold::CAP,
            },
        })
    );
}

#[test]
fn a_pile_entry_over_ninety_nine_units_is_refused() {
    let mut save = marker();
    let index = save
        .tabs
        .iter()
        .position(|tab| matches!(tab.body, TabBody::AdvancedStash(_)))
        .expect("an AdvancedStash tab");
    let TabBody::AdvancedStash(items) = &mut save.tabs[index].body else {
        unreachable!()
    };
    let stack = Some(Bits::<8>::new(u32::from(PileCount::MAX) + 1).expect("stack"));
    match &mut items.0[0].body {
        ItemBody::Compact(body) => body.advanced_stash_stack = stack,
        ItemBody::Complete(body) => body.advanced_stash_stack = stack,
    }
    let index = u8::try_from(index).expect("tab index");
    assert_eq!(
        build(&save),
        Err(VaultError::Pile {
            container: ContainerId::Pile(pile_tab(index)),
            source: ValueError::PileCountOverCap {
                count: u16::from(PileCount::MAX) + 1,
                max: PileCount::MAX,
            },
        })
    );
}

#[test]
fn an_item_whose_flags_and_payload_disagree_is_refused() {
    let mut save = marker();
    let (index, items) = first_normal(&mut save);
    items[0].flags = ItemFlags::from_bits(items[0].flags.bits() | ItemFlags::IS_EAR);
    let index = u8::try_from(index).expect("tab index");
    let error = build(&save).expect_err("the item cannot be written");
    assert!(
        matches!(
            &error,
            VaultError::Object {
                container,
                source: ModelError::Format(_),
            } if *container == ContainerId::Grid(tab(index))
        ),
        "{error}"
    );
}

#[test]
fn a_container_declared_twice_is_refused() {
    let twice = Vault::builder()
        .grid(&tab(0), vec![])
        .expect("first")
        .grid(&tab(0), vec![]);
    assert_eq!(
        twice.err(),
        Some(VaultError::ContainerDeclaredTwice {
            container: ContainerId::Grid(tab(0)),
        })
    );
}

#[test]
fn a_stash_with_more_tabs_than_an_index_carries_is_refused() {
    let mut save = marker();
    let first = save.tabs[0].clone();
    while u8::try_from(save.tabs.len()).is_ok() {
        save.tabs.push(first.clone());
    }
    let tabs = save.tabs.len();
    assert_eq!(
        build(&save),
        Err(VaultError::TooManyTabs {
            stash: stash(),
            tabs,
            max: u8::MAX,
        })
    );
}

#[test]
fn applying_against_a_vault_missing_a_container_is_refused() {
    let rune = compact("r08");
    let vault = Vault::builder()
        .grid(&tab(0), vec![(at(0, 0), object(&rune))])
        .expect("source")
        .grid(&tab(1), vec![])
        .expect("target")
        .build();

    let validated = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(0),
            fingerprint: object(&rune).fingerprint(),
        },
        Destination::Grid {
            container: tab(1),
            cell: None,
        },
    )])
    .validate(vault)
    .expect("the plan is accepted");

    let short = Vault::builder().grid(&tab(0), vec![]).expect("tab").build();
    assert_eq!(
        validated.apply(&short),
        Err(ApplyError::ContainerMissing {
            container: ContainerId::Grid(tab(1)),
        })
    );
}

#[test]
fn applying_against_a_vault_holding_an_unexpected_container_is_refused() {
    let vault = Vault::builder().grid(&tab(0), vec![]).expect("tab").build();
    let validated = Plan::new(vec![])
        .validate(vault)
        .expect("an empty plan is accepted");

    let wider = Vault::builder()
        .grid(&tab(0), vec![])
        .expect("tab")
        .pile(&pile_tab(5), vec![])
        .expect("pile")
        .build();
    assert_eq!(
        validated.apply(&wider),
        Err(ApplyError::ContainerUnexpected {
            container: ContainerId::Pile(pile_tab(5)),
        })
    );
}

#[test]
fn a_census_reports_the_items_a_write_lost_or_grew() {
    let rune = compact("r08");
    let other = compact_marked("r08", 1);
    let before = Census::of(
        &Vault::builder()
            .grid(&tab(0), vec![(at(0, 0), object(&rune))])
            .expect("tab")
            .build(),
    );
    let lost = Census::of(&Vault::builder().grid(&tab(0), vec![]).expect("tab").build());
    let grown = Census::of(
        &Vault::builder()
            .grid(
                &tab(0),
                vec![(at(0, 0), object(&rune)), (at(1, 0), object(&other))],
            )
            .expect("tab")
            .build(),
    );
    let none = domain::census::Units::new();

    assert_eq!(before.conserved(&before, &none), Ok(()));

    let missing = before
        .conserved(&lost, &none)
        .expect_err("a lost item breaks the multiset");
    assert_eq!(
        missing.missing,
        [(object(&rune).fingerprint(), 1)].into_iter().collect()
    );
    assert!(missing.appeared.is_empty());

    let appeared = before
        .conserved(&grown, &none)
        .expect_err("an item out of nowhere breaks the multiset");
    assert_eq!(
        appeared.appeared,
        [(object(&other).fingerprint(), 1)].into_iter().collect()
    );
    assert!(appeared.missing.is_empty());

    let declared = [(object(&other).fingerprint(), 1)].into_iter().collect();
    assert_eq!(before.conserved(&grown, &declared), Ok(()));
}

#[test]
fn a_vault_gives_its_items_back_for_writing() {
    let save = marker();
    let vault = build(&save).expect("vault");

    let mut grid_items: Vec<Item> = Vec::new();
    for (_, grid) in vault.grids().map(|(id, grid)| (id, grid.clone())) {
        for slot in grid.into_slots() {
            let origin = slot.origin();
            let mut item = slot.into_object().into_item();
            if let ItemPosition::Slot { cell, .. } = &mut item.position {
                *cell = origin.to_cell();
            }
            grid_items.push(item);
        }
    }

    let mut pile_items: Vec<Item> = Vec::new();
    for (_, pile) in vault.piles().map(|(id, pile)| (id, pile.clone())) {
        for entry in pile.into_entries() {
            let count = Bits::<8>::new(u32::from(entry.count().get())).expect("stack");
            let mut item = entry.into_object().into_item();
            match &mut item.body {
                ItemBody::Compact(body) => body.advanced_stash_stack = Some(count),
                ItemBody::Complete(body) => body.advanced_stash_stack = Some(count),
            }
            pile_items.push(item);
        }
    }

    assert_eq!(grid_items.len(), 146);
    assert_eq!(pile_items.len(), 81);

    let mut original: Vec<&Item> = Vec::new();
    for tab in &save.tabs {
        if let TabBody::Normal(items) | TabBody::AdvancedStash(items) = &tab.body {
            original.extend(items.0.iter());
        }
    }
    let mut rebuilt: Vec<&Item> = grid_items.iter().chain(pile_items.iter()).collect();
    rebuilt.sort_by_key(|item| format!("{item:?}"));
    original.sort_by_key(|item| format!("{item:?}"));
    assert_eq!(rebuilt, original, "the vault gives back what the file held");
}

#[test]
fn a_pile_never_hands_out_more_units_than_it_holds() {
    let rune = compact("r08");
    let vault = Vault::builder()
        .pile(&pile_tab(5), vec![(object(&rune), PileCount::ZERO)])
        .expect("pile")
        .pile(&pile_tab(6), vec![])
        .expect("target")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Pile {
            container: pile_tab(5),
            fingerprint: object(&rune).fingerprint(),
            units: PileUnits::ONE,
        },
        Destination::Pile {
            container: pile_tab(6),
        },
    )]);

    assert_eq!(
        plan.validate(vault)
            .expect_err("nothing to hand over")
            .violation,
        RuleViolation::NotEnoughUnits {
            container: ContainerId::Pile(pile_tab(5)),
            fingerprint: object(&rune).fingerprint(),
            held: 0,
            asked: PileUnits::ONE,
        }
    );
}
