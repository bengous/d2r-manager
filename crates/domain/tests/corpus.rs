mod support;

use std::collections::BTreeMap;

use domain::census::Census;
use domain::container::{ContainerId, GridId, PileId, TabIndex};
use domain::tables::CarryGroup;
use domain::vault::{ContainerRef, Vault};

use format::save::CharacterSave;
use format::stash::StashSave;

use support::{character_name, fixture_bytes, fixture_names, stash_name};

const LINUX: &str = "v105/linux";
const WINDOWS: &str = "v105/windows";
const MARKER: &str = "ModernSharedStashSoftCoreV2.d2i";

fn vault_of(directory: &str, name: &str) -> Vault {
    let bytes = fixture_bytes(&format!("{directory}/{name}"));
    let builder = Vault::builder();
    if support::extension(name) == Some("d2i") {
        let save = StashSave::read(&bytes).expect("shared stash");
        builder
            .shared_stash(&stash_name(name), &save)
            .expect("stash vault")
            .build()
    } else {
        let save = CharacterSave::read(&bytes).expect("character save");
        let stem = name.trim_end_matches(".d2s");
        builder
            .character(&character_name(stem), &save)
            .expect("character vault")
            .build()
    }
}

#[test]
fn every_version_105_fixture_builds_a_vault() {
    let mut seen = 0;
    for directory in [LINUX, WINDOWS] {
        for name in fixture_names(directory) {
            let vault = vault_of(directory, &name);
            for (id, container) in vault.containers() {
                match container {
                    ContainerRef::Grid(grid) => {
                        for slot in grid.slots() {
                            let footprint = slot.object().footprint();
                            assert!(
                                u16::from(slot.origin().column().get())
                                    + u16::from(footprint.width())
                                    <= u16::from(grid.geometry().columns())
                                    && u16::from(slot.origin().row().get())
                                        + u16::from(footprint.height())
                                        <= u16::from(grid.geometry().rows()),
                                "{directory}/{name}: {id} holds an item outside its grid"
                            );
                        }
                    }
                    ContainerRef::Pile(_) => {}
                }
            }
            seen += 1;
        }
    }
    assert_eq!(seen, 57);
}

#[test]
fn the_shared_stash_marker_file_holds_the_counts_measured_on_it() {
    let vault = vault_of(LINUX, MARKER);
    let stash = stash_name(MARKER);

    let grids: Vec<_> = vault.grids().collect();
    let piles: Vec<_> = vault.piles().collect();
    assert_eq!(grids.len(), 5, "five normal tabs");
    assert_eq!(piles.len(), 1, "one AdvancedStash pile");

    let entries: usize = grids
        .iter()
        .map(|(_, grid)| grid.slots().len())
        .chain(piles.iter().map(|(_, pile)| pile.entries().len()))
        .sum();
    assert_eq!(entries, 227, "227 item entries");

    let units: u32 = piles
        .iter()
        .flat_map(|(_, pile)| pile.entries())
        .map(|entry| u32::from(entry.count().get()))
        .sum();
    assert_eq!(units, 1306, "the AdvancedStash piles hold 1306 units");

    let first_tab = ContainerId::Grid(GridId::SharedTab {
        stash: stash.clone(),
        tab: TabIndex::new(0),
    });
    assert_eq!(
        vault.gold(&first_tab).map(domain::gold::Gold::get),
        Some(1_075_411),
        "1 075 411 gold on the first tab"
    );

    let census = Census::of(&vault);
    let counted: u32 = census.totals().values().sum();
    assert_eq!(counted, 146 + 1306);

    let pile_id = PileId::new(stash, TabIndex::new(5));
    assert_eq!(vault.pile(&pile_id).expect("pile").entries().len(), 81);
}

#[test]
fn no_character_of_the_corpus_carries_two_charms_of_one_exclusive_group() {
    let mut checked = 0;
    let mut carriers = 0;
    for name in fixture_names(LINUX) {
        if support::extension(&name) != Some("d2s") {
            continue;
        }
        let vault = vault_of(LINUX, &name);
        let mut held: BTreeMap<CarryGroup, u32> = BTreeMap::new();
        for (_, grid) in vault.grids() {
            for slot in grid.slots() {
                if let Some(group) = slot.object().carry_group() {
                    *held.entry(group).or_default() += 1;
                }
            }
        }
        for (group, count) in &held {
            assert!(
                *count <= 1,
                "{name} carries {count} items of the exclusive group {}",
                group.get()
            );
        }
        if !held.is_empty() {
            carriers += 1;
        }
        checked += 1;
    }
    assert_eq!(checked, 53, "53 characters in the frozen corpus");
    assert!(carriers > 0, "some characters do carry an exclusive charm");
}

#[test]
fn the_shared_stash_carries_more_exclusive_charms_than_a_character_may() {
    let vault = vault_of(LINUX, MARKER);
    let mut held: BTreeMap<CarryGroup, u32> = BTreeMap::new();
    for (_, grid) in vault.grids() {
        for slot in grid.slots() {
            if let Some(group) = slot.object().carry_group() {
                *held.entry(group).or_default() += 1;
            }
        }
    }
    assert!(
        held.values().any(|count| *count > 1),
        "the marker stash holds several charms of one exclusive group: {held:?}"
    );
}
