mod support;

use std::collections::BTreeMap;

use domain::census::Census;
use domain::container::{ContainerId, GridId, PileId, StashName, TabIndex};
use domain::gold::Gold;
use domain::grid::GridPosition;
use domain::object::Fingerprint;
use domain::pile::PileUnits;
use domain::plan::{Destination, Operation, Plan, Source};
use domain::vault::Vault;

use format::stash::StashSave;

use support::{fixture_bytes, merge_bytes, stash_name};

const WINDOWS_STASH: &str = "v105/windows/ModernSharedStashSoftCoreV2.d2i";
const NORMAL_TABS: [u8; 5] = [0, 1, 2, 3, 4];
const PILE_TAB: u8 = 5;

fn linux() -> StashName {
    stash_name("linux")
}

fn windows() -> StashName {
    stash_name("windows")
}

fn tab(stash: &StashName, index: u8) -> GridId {
    GridId::SharedTab {
        stash: stash.clone(),
        tab: TabIndex::new(index),
    }
}

fn pile(stash: &StashName, index: u8) -> PileId {
    PileId::new(stash.clone(), TabIndex::new(index))
}

fn read(bytes: &[u8]) -> StashSave {
    StashSave::read(bytes).expect("shared stash")
}

fn single(name: &StashName, save: &StashSave) -> Vault {
    Vault::builder()
        .shared_stash(name, save)
        .expect("stash vault")
        .build()
}

fn both() -> Vault {
    Vault::builder()
        .shared_stash(&linux(), &read(&merge_bytes("before.d2i")))
        .expect("linux stash")
        .shared_stash(&windows(), &read(&fixture_bytes(WINDOWS_STASH)))
        .expect("windows stash")
        .build()
}

fn reference() -> Vault {
    single(&linux(), &read(&merge_bytes("after.d2i")))
}

fn placements(vault: &Vault, id: &GridId) -> Vec<(Fingerprint, GridPosition)> {
    vault
        .grid(id)
        .expect("grid")
        .slots()
        .iter()
        .map(|slot| (slot.object().fingerprint(), slot.origin()))
        .collect()
}

fn pile_entries(vault: &Vault, id: &PileId) -> Vec<(Fingerprint, u8, u32, u32)> {
    vault
        .pile(id)
        .expect("pile")
        .entries()
        .iter()
        .map(|entry| {
            let (column, row) = match entry.object().item().position {
                format::item::ItemPosition::Slot { cell, .. } => (cell.column(), cell.row()),
                format::item::ItemPosition::Ground { x, y, .. } => (u32::from(x), u32::from(y)),
            };
            (
                entry.object().fingerprint(),
                entry.count().get(),
                column,
                row,
            )
        })
        .collect()
}

fn multiset<T: Ord>(values: impl IntoIterator<Item = T>) -> BTreeMap<T, u32> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
}

fn added_cells(before: &Vault, after: &Vault) -> BTreeMap<u8, Vec<(Fingerprint, GridPosition)>> {
    let mut added = BTreeMap::new();
    for index in NORMAL_TABS {
        let mut held = multiset(placements(before, &tab(&linux(), index)));
        let mut new_cells = Vec::new();
        for placement in placements(after, &tab(&linux(), index)) {
            match held.get_mut(&placement) {
                Some(count) if *count > 0 => *count -= 1,
                _ => new_cells.push(placement),
            }
        }
        assert!(
            held.values().all(|count| *count == 0),
            "tab {index}: the merge moved or dropped an item that was already there"
        );
        added.insert(index, new_cells);
    }
    added
}

fn windows_grid_items(vault: &Vault) -> Vec<(u8, Fingerprint)> {
    NORMAL_TABS
        .iter()
        .flat_map(|index| {
            placements(vault, &tab(&windows(), *index))
                .into_iter()
                .map(|(fingerprint, _)| (*index, fingerprint))
        })
        .collect()
}

/// The plan: move every Windows item into the Linux stash. Grid items land on
/// the cell the reference merge gave them; pile units land by the stacking
/// rule alone, with no cell named anywhere.
fn pile_operations(vault: &Vault) -> Vec<Operation> {
    vault
        .pile(&pile(&windows(), PILE_TAB))
        .expect("pile")
        .entries()
        .iter()
        .map(|entry| {
            let units =
                PileUnits::new(entry.count().get()).expect("the Windows pile holds no empty entry");
            Operation::moved(
                Source::Pile {
                    container: pile(&windows(), PILE_TAB),
                    fingerprint: entry.object().fingerprint(),
                    units,
                },
                Destination::Pile {
                    container: pile(&linux(), PILE_TAB),
                },
            )
        })
        .collect()
}

fn plan_on_the_reference_cells(
    vault: &Vault,
    cells: &BTreeMap<u8, Vec<(Fingerprint, GridPosition)>>,
) -> Plan {
    let mut free: BTreeMap<Fingerprint, Vec<(u8, GridPosition)>> = BTreeMap::new();
    for (index, placements) in cells {
        for (fingerprint, cell) in placements {
            free.entry(*fingerprint).or_default().push((*index, *cell));
        }
    }

    let mut operations = Vec::new();
    for (source_tab, fingerprint) in windows_grid_items(vault) {
        let (target_tab, cell) = free
            .get_mut(&fingerprint)
            .and_then(Vec::pop)
            .expect("the reference merge kept every Windows item");
        operations.push(Operation::moved(
            Source::Grid {
                container: tab(&windows(), source_tab),
                fingerprint,
            },
            Destination::Grid {
                container: tab(&linux(), target_tab),
                cell: Some(cell),
            },
        ));
    }
    assert!(
        free.values().all(Vec::is_empty),
        "the reference merge added an item the Windows stash never held"
    );
    operations.extend(pile_operations(vault));
    Plan::new(operations)
}

fn plan_on_the_first_free_tab(vault: &Vault) -> Plan {
    let mut working = vault.clone();
    let mut operations = Vec::new();
    for (source_tab, fingerprint) in windows_grid_items(vault) {
        let source = tab(&windows(), source_tab);
        let footprint = working
            .grid(&source)
            .expect("windows tab")
            .find(fingerprint)
            .expect("the item is still there")
            .footprint();
        let target_tab = NORMAL_TABS
            .into_iter()
            .find(|index| {
                working
                    .grid(&tab(&linux(), *index))
                    .expect("linux tab")
                    .first_free(footprint)
                    .is_some()
            })
            .expect("the Linux stash has room for every Windows item");
        let operation = Operation::moved(
            Source::Grid {
                container: source,
                fingerprint,
            },
            Destination::Grid {
                container: tab(&linux(), target_tab),
                cell: None,
            },
        );
        working = Plan::new(vec![operation.clone()])
            .validate(working)
            .expect("the step is valid")
            .outcome()
            .clone();
        operations.push(operation);
    }
    operations.extend(pile_operations(vault));
    Plan::new(operations)
}

/// Equivalence, as this test reads it: the merged Linux stash holds, container
/// by container, the same items as the file the game wrote. For the grid tabs
/// that means the same multiset of fingerprints at the same cells, because the
/// plan names the cells the reference chose. For the `AdvancedStash` pile it
/// means the same entries in the same order, each with the same fingerprint,
/// the same unit count, and the same position, because the pile rule invents
/// no position: it stacks up to 99 units, then adds an entry that keeps the
/// position the incoming item already carried.
#[test]
fn the_stash_merge_of_2026_08_20_replays_to_the_file_the_game_wrote() {
    let start = both();
    let after = reference();
    let cells = added_cells(&single(&linux(), &read(&merge_bytes("before.d2i"))), &after);

    let plan = plan_on_the_reference_cells(&start, &cells);
    assert_eq!(plan.operations().len(), 67 + 68);

    let validated = plan.validate(start).expect("the merge is a valid plan");
    assert!(
        validated.conservation().is_held(),
        "{:?}",
        validated.conservation()
    );
    let outcome = validated.outcome();

    for index in NORMAL_TABS {
        assert_eq!(
            multiset(placements(outcome, &tab(&linux(), index))),
            multiset(placements(&after, &tab(&linux(), index))),
            "linux tab {index}"
        );
        assert!(
            placements(outcome, &tab(&windows(), index)).is_empty(),
            "windows tab {index} still holds items"
        );
    }

    assert_eq!(
        pile_entries(outcome, &pile(&linux(), PILE_TAB)),
        pile_entries(&after, &pile(&linux(), PILE_TAB))
    );
    assert!(
        outcome
            .pile(&pile(&windows(), PILE_TAB))
            .expect("windows pile")
            .entries()
            .is_empty()
    );
}

#[test]
fn the_merged_stash_holds_the_same_items_when_the_plan_names_no_cell() {
    let start = both();
    let after = reference();

    let plan = plan_on_the_first_free_tab(&start);
    let validated = plan.validate(start).expect("the merge is a valid plan");
    let outcome = validated.outcome();

    let held = Census::of(outcome).totals();
    let wanted = Census::of(&after).totals();
    let mut differences: Vec<(Fingerprint, u32, u32)> = Vec::new();
    for (fingerprint, count) in &held {
        let other = wanted.get(fingerprint).copied().unwrap_or_default();
        if *count != other {
            differences.push((*fingerprint, *count, other));
        }
    }
    for (fingerprint, count) in &wanted {
        if !held.contains_key(fingerprint) {
            differences.push((*fingerprint, 0, *count));
        }
    }
    assert!(differences.is_empty(), "{differences:?}");

    for index in NORMAL_TABS {
        let grid = outcome.grid(&tab(&linux(), index)).expect("linux tab");
        let mut cells: Vec<(u8, u8)> = Vec::new();
        for slot in grid.slots() {
            let footprint = slot.object().footprint();
            for row in slot.origin().row()..slot.origin().row() + footprint.height() {
                for column in slot.origin().column()..slot.origin().column() + footprint.width() {
                    assert!(
                        column < grid.geometry().columns() && row < grid.geometry().rows(),
                        "tab {index}: an item leaves the grid"
                    );
                    cells.push((column, row));
                }
            }
        }
        let taken = cells.len();
        cells.sort_unstable();
        cells.dedup();
        assert_eq!(taken, cells.len(), "tab {index}: two items share a cell");
    }
}

#[test]
fn the_merge_sums_the_gold_of_the_two_stashes_below_the_cap() {
    let before = read(&merge_bytes("before.d2i"));
    let source = read(&fixture_bytes(WINDOWS_STASH));
    let after = read(&merge_bytes("after.d2i"));

    let held = Gold::new(before.tabs[0].gold).expect("linux gold");
    let incoming = Gold::new(source.tabs[0].gold).expect("windows gold");
    let expected = Gold::new(after.tabs[0].gold).expect("merged gold");

    assert_eq!(held.get(), 135_411);
    assert_eq!(incoming.get(), 940_000);
    assert_eq!(held.try_add(incoming), Ok(expected));
    assert_eq!(expected.get(), 1_075_411);

    let vault = single(&linux(), &after);
    assert_eq!(
        vault.gold(&ContainerId::Grid(tab(&linux(), 0))),
        Some(expected)
    );
}

#[test]
fn the_reference_merge_moved_no_item_the_windows_stash_did_not_hold() {
    let start = both();
    let before = single(&linux(), &read(&merge_bytes("before.d2i")));
    let after = reference();

    let mut incoming: BTreeMap<Fingerprint, u32> = BTreeMap::new();
    for (_, fingerprint) in windows_grid_items(&start) {
        *incoming.entry(fingerprint).or_default() += 1;
    }
    for entry in start
        .pile(&pile(&windows(), PILE_TAB))
        .expect("pile")
        .entries()
    {
        *incoming.entry(entry.object().fingerprint()).or_default() +=
            u32::from(entry.count().get());
    }

    let mut expected = Census::of(&before).totals();
    for (fingerprint, count) in &incoming {
        *expected.entry(*fingerprint).or_default() += count;
    }
    assert_eq!(expected, Census::of(&after).totals());
}
