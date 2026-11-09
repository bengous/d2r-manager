mod support;

use domain::container::{CharacterPage, ContainerId, GridId, PileId, TabIndex};
use domain::error::{ApplyError, RuleViolation, ValueError};
use domain::gold::Gold;
use domain::grid::{Column, GridPosition, Row};
use domain::object::Object;
use domain::pile::{PileCount, PileUnits};
use domain::plan::{Destination, Landing, Operation, Plan, Source, Transfer};
use domain::tables::{self, CarryGroup, UniqueId};
use domain::vault::{Vault, VaultBuilder};

use support::{character_name, compact, compact_marked, place, stash_name, unique};

const ANNIHILUS: u32 = 381;
const HELLFIRE_TORCH: u32 = 400;
const GHEEDS_FORTUNE: u32 = 359;
const COLD_RUPTURE: u32 = 427;
const FLAME_RIFT: u32 = 433;

fn tab(index: u8) -> GridId {
    GridId::SharedTab {
        stash: stash_name("shared"),
        tab: TabIndex::new(index),
    }
}

fn pile_tab(index: u8) -> PileId {
    PileId::new(stash_name("shared"), TabIndex::new(index))
}

fn page(page: CharacterPage) -> GridId {
    GridId::Character {
        character: character_name("Elektra"),
        page,
    }
}

fn at(column: u8, row: u8) -> GridPosition {
    GridPosition::new(Column::new(column), Row::new(row)).expect("grid position")
}

fn object(item: &format::item::Item) -> Object {
    Object::new(item.clone()).expect("object")
}

fn units(count: u8) -> PileUnits {
    PileUnits::new(count).expect("pile units")
}

fn count(value: u8) -> PileCount {
    PileCount::new(value).expect("pile count")
}

fn builder() -> VaultBuilder {
    Vault::builder()
}

fn refuse(plan: Plan, vault: Vault) -> RuleViolation {
    plan.validate(vault)
        .expect_err("the plan is refused")
        .violation
}

fn accept(plan: Plan, vault: Vault) -> Vault {
    let validated = plan.validate(vault).expect("the plan is accepted");
    validated
        .conserved()
        .expect("a valid plan keeps the multiset");
    validated.into_outcome()
}

#[test]
fn item_sizes_come_from_the_version_105_tables() {
    let table = [
        ("r08", 1, 1),
        ("cm1", 1, 1),
        ("cm2", 1, 2),
        ("cm3", 1, 3),
        ("cs2", 1, 3),
        ("ear", 1, 1),
        ("gld", 1, 1),
        ("am1", 2, 4),
        ("pk1", 1, 2),
    ];
    for (code, width, height) in table {
        let code = format::code::ItemCode::from_text(code).expect("item code");
        let footprint = tables::footprint(code).expect("footprint");
        assert_eq!(
            (footprint.width(), footprint.height()),
            (width, height),
            "{code}"
        );
    }
}

#[test]
fn an_unknown_item_code_has_no_size() {
    let code = format::code::ItemCode::from_text("zzzz").expect("item code");
    assert_eq!(
        tables::footprint(code),
        Err(domain::error::ModelError::UnknownItemSize { code })
    );
}

#[test]
fn a_grid_takes_an_item_that_fits() {
    let vault = builder()
        .grid(&tab(0), vec![])
        .expect("empty tab")
        .grid(&tab(1), vec![(at(0, 0), object(&compact("r08")))])
        .expect("tab with a rune")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(1),
            fingerprint: object(&compact("r08")).fingerprint(),
        },
        Destination::Grid {
            container: tab(0),
            cell: Some(at(3, 4)),
        },
    )]);

    let outcome = accept(plan, vault);
    let target = outcome.grid(&tab(0)).expect("target tab");
    assert_eq!(target.slots().len(), 1);
    assert_eq!(target.slots()[0].origin(), at(3, 4));
    assert_eq!(outcome.grid(&tab(1)).expect("source tab").slots(), []);
}

#[test]
fn a_grid_refuses_an_item_that_leaves_it() {
    let table = [
        (page(CharacterPage::Inventory), "cm3", at(0, 2)),
        (page(CharacterPage::Cube), "am1", at(0, 1)),
        (tab(0), "cm3", at(9, 8)),
    ];
    for (target, code, cell) in table {
        let vault = builder()
            .grid(&target, vec![])
            .expect("target")
            .grid(&tab(5), vec![(at(0, 0), object(&compact(code)))])
            .expect("source")
            .build();
        let plan = Plan::new(vec![Operation::moved(
            Source::Grid {
                container: tab(5),
                fingerprint: object(&compact(code)).fingerprint(),
            },
            Destination::Grid {
                container: target.clone(),
                cell: Some(cell),
            },
        )]);
        let violation = refuse(plan, vault);
        assert!(
            matches!(violation, RuleViolation::OutsideGrid { .. }),
            "{target} {code}: {violation}"
        );
    }
}

#[test]
fn a_grid_refuses_a_cell_another_item_already_covers() {
    let vault = builder()
        .grid(&tab(0), vec![(at(2, 2), object(&compact("cm3")))])
        .expect("target")
        .grid(&tab(1), vec![(at(0, 0), object(&compact("r08")))])
        .expect("source")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(1),
            fingerprint: object(&compact("r08")).fingerprint(),
        },
        Destination::Grid {
            container: tab(0),
            cell: Some(at(2, 4)),
        },
    )]);

    assert_eq!(
        refuse(plan, vault),
        RuleViolation::CellTaken {
            container: tab(0),
            cell: at(2, 4),
        }
    );
}

#[test]
fn a_full_grid_has_no_cell_left_to_append_to() {
    let cube = page(CharacterPage::Cube);
    let filled: Vec<_> = (0..12)
        .map(|index| {
            let column = index % 3;
            let row = index / 3;
            (
                at(column, row),
                object(&compact_marked("r08", index.into())),
            )
        })
        .collect();
    let vault = builder()
        .grid(&cube, filled)
        .expect("full cube")
        .grid(&tab(0), vec![(at(0, 0), object(&compact("r09")))])
        .expect("source")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(0),
            fingerprint: object(&compact("r09")).fingerprint(),
        },
        Destination::Grid {
            container: cube.clone(),
            cell: None,
        },
    )]);

    assert_eq!(
        refuse(plan, vault),
        RuleViolation::GridFull {
            container: cube,
            footprint: tables::footprint(
                format::code::ItemCode::from_text("r09").expect("item code")
            )
            .expect("footprint"),
        }
    );
}

#[test]
fn an_empty_cell_is_chosen_row_by_row_when_the_plan_names_none() {
    let vault = builder()
        .grid(
            &tab(0),
            vec![
                (at(0, 0), object(&compact_marked("r08", 1))),
                (at(1, 0), object(&compact_marked("r08", 2))),
            ],
        )
        .expect("target")
        .grid(&tab(1), vec![(at(5, 5), object(&compact("r09")))])
        .expect("source")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(1),
            fingerprint: object(&compact("r09")).fingerprint(),
        },
        Destination::Grid {
            container: tab(0),
            cell: None,
        },
    )]);

    let validated = plan.validate(vault).expect("the plan is accepted");
    assert_eq!(validated.realized()[0].landing(), Landing::Cell(at(2, 0)));
}

#[test]
fn gold_stops_at_two_million_five_hundred_thousand_a_tab() {
    assert_eq!(Gold::CAP, 2_500_000);
    assert_eq!(Gold::new(2_500_000).map(Gold::get), Ok(2_500_000));
    assert_eq!(
        Gold::new(2_500_001),
        Err(ValueError::GoldOverCap {
            amount: 2_500_001,
            cap: 2_500_000,
        })
    );
}

#[test]
fn gold_added_across_the_cap_is_refused() {
    let held = Gold::new(1_075_411).expect("gold");
    let added = Gold::new(940_000).expect("gold");
    assert_eq!(held.try_add(added).map(Gold::get), Ok(2_015_411));
    assert_eq!(
        held.try_add(Gold::new(1_500_000).expect("gold")),
        Err(ValueError::GoldOverCap {
            amount: 2_575_411,
            cap: 2_500_000,
        })
    );
}

#[test]
fn a_pile_stops_at_ninety_nine_units() {
    assert_eq!(PileCount::MAX, 99);
    assert_eq!(PileCount::new(99).map(PileCount::get), Ok(99));
    assert_eq!(
        PileCount::new(100),
        Err(ValueError::PileCountOverCap {
            count: 100,
            max: 99,
        })
    );
    assert_eq!(
        PileUnits::new(0),
        Err(ValueError::PileCountOverCap { count: 0, max: 99 })
    );
}

#[test]
fn a_pile_stacks_up_to_ninety_nine_then_adds_a_second_entry() {
    let rune = compact("r08");
    let vault = builder()
        .pile(&pile_tab(5), vec![(object(&rune), count(12))])
        .expect("target pile")
        .pile(&pile_tab(6), vec![(object(&rune), count(95))])
        .expect("source pile")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Pile {
            container: pile_tab(6),
            fingerprint: object(&rune).fingerprint(),
            units: units(95),
        },
        Destination::Pile {
            container: pile_tab(5),
        },
    )]);

    let outcome = accept(plan, vault);
    let pile = outcome.pile(&pile_tab(5)).expect("target pile");
    let counts: Vec<u8> = pile.entries().iter().map(|e| e.count().get()).collect();
    assert_eq!(counts, vec![99, 8]);
    assert_eq!(outcome.pile(&pile_tab(6)).expect("source").entries(), []);
}

#[test]
fn a_pile_adds_an_entry_when_no_stack_holds_the_same_item() {
    let mine = compact_marked("pk1", 1);
    let other = compact_marked("pk1", 2);
    let vault = builder()
        .pile(&pile_tab(5), vec![(object(&mine), count(2))])
        .expect("target pile")
        .pile(&pile_tab(6), vec![(object(&other), count(1))])
        .expect("source pile")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Pile {
            container: pile_tab(6),
            fingerprint: object(&other).fingerprint(),
            units: units(1),
        },
        Destination::Pile {
            container: pile_tab(5),
        },
    )]);

    let outcome = accept(plan, vault);
    let pile = outcome.pile(&pile_tab(5)).expect("target pile");
    assert_eq!(pile.entries().len(), 2);
    assert_eq!(pile.units(object(&mine).fingerprint()), 2);
    assert_eq!(pile.units(object(&other).fingerprint()), 1);
}

#[test]
fn a_pile_refuses_to_hand_over_units_it_does_not_hold() {
    let rune = compact("r08");
    let vault = builder()
        .pile(&pile_tab(5), vec![(object(&rune), count(3))])
        .expect("source pile")
        .pile(&pile_tab(6), vec![])
        .expect("target pile")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Pile {
            container: pile_tab(5),
            fingerprint: object(&rune).fingerprint(),
            units: units(4),
        },
        Destination::Pile {
            container: pile_tab(6),
        },
    )]);

    assert_eq!(
        refuse(plan, vault),
        RuleViolation::NotEnoughUnits {
            container: ContainerId::Pile(pile_tab(5)),
            fingerprint: object(&rune).fingerprint(),
            held: 3,
            asked: units(4),
        }
    );
}

#[test]
fn a_grid_destination_takes_a_single_unit_at_a_time() {
    let rune = compact("r08");
    let vault = builder()
        .pile(&pile_tab(5), vec![(object(&rune), count(9))])
        .expect("source pile")
        .grid(&tab(0), vec![])
        .expect("target tab")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Pile {
            container: pile_tab(5),
            fingerprint: object(&rune).fingerprint(),
            units: units(2),
        },
        Destination::Grid {
            container: tab(0),
            cell: Some(at(0, 0)),
        },
    )]);

    assert_eq!(
        refuse(plan, vault),
        RuleViolation::GridTakesOneUnit {
            container: tab(0),
            asked: units(2),
        }
    );
}

#[test]
fn the_exclusive_charms_carry_their_group_from_the_txt_files() {
    let table = [
        (ANNIHILUS, 1002),
        (HELLFIRE_TORCH, 1003),
        (GHEEDS_FORTUNE, 1001),
        (COLD_RUPTURE, 1004),
        (FLAME_RIFT, 1004),
    ];
    for (unique_id, group) in table {
        assert_eq!(
            tables::carry_group(UniqueId::new(u16::try_from(unique_id).expect("unique id"))),
            Some(CarryGroup::new(group)),
            "unique {unique_id}"
        );
    }
    assert_eq!(tables::carry_group(UniqueId::new(0)), None);
}

#[test]
fn a_character_takes_one_exclusive_charm() {
    let charm = unique("cm1", ANNIHILUS);
    let inventory = page(CharacterPage::Inventory);
    let vault = builder()
        .grid(&inventory, vec![])
        .expect("inventory")
        .grid(&tab(0), vec![(at(0, 0), object(&charm))])
        .expect("shared tab")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(0),
            fingerprint: object(&charm).fingerprint(),
        },
        Destination::Grid {
            container: inventory.clone(),
            cell: Some(at(0, 0)),
        },
    )]);

    let outcome = accept(plan, vault);
    assert_eq!(
        outcome.grid(&inventory).expect("inventory").slots().len(),
        1
    );
}

#[test]
fn a_character_refuses_a_second_charm_of_the_same_exclusive_group() {
    let table = [
        ("cm1", ANNIHILUS, 1002),
        ("cm2", HELLFIRE_TORCH, 1003),
        ("cm3", GHEEDS_FORTUNE, 1001),
    ];
    for (code, unique_id, group) in table {
        let held = unique(code, unique_id);
        let incoming = place(&held, 1, 0);
        let inventory = page(CharacterPage::Inventory);
        let vault = builder()
            .grid(&inventory, vec![(at(0, 0), object(&held))])
            .expect("inventory")
            .grid(&tab(0), vec![(at(0, 0), object(&incoming))])
            .expect("shared tab")
            .build();

        let plan = Plan::new(vec![Operation::duplicated(
            Source::Grid {
                container: tab(0),
                fingerprint: object(&incoming).fingerprint(),
            },
            Destination::Grid {
                container: inventory.clone(),
                cell: Some(at(4, 0)),
            },
        )]);

        assert_eq!(
            refuse(plan, vault),
            RuleViolation::CarryLimitExceeded {
                character: character_name("Elektra"),
                group: CarryGroup::new(group),
                count: 2,
            },
            "{code}"
        );
    }
}

#[test]
fn the_shared_stash_holds_as_many_exclusive_charms_as_it_likes() {
    let first = unique("cm2", HELLFIRE_TORCH);
    let second = place(&first, 2, 0);
    let vault = builder()
        .grid(&tab(0), vec![(at(0, 0), object(&first))])
        .expect("target tab")
        .grid(&tab(1), vec![(at(0, 0), object(&second))])
        .expect("source tab")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(1),
            fingerprint: object(&second).fingerprint(),
        },
        Destination::Grid {
            container: tab(0),
            cell: Some(at(4, 0)),
        },
    )]);

    let outcome = accept(plan, vault);
    assert_eq!(outcome.grid(&tab(0)).expect("tab").slots().len(), 2);
}

#[test]
fn two_charms_of_different_exclusive_groups_live_together() {
    let annihilus = unique("cm1", ANNIHILUS);
    let torch = unique("cm2", HELLFIRE_TORCH);
    let inventory = page(CharacterPage::Inventory);
    let vault = builder()
        .grid(&inventory, vec![(at(0, 0), object(&annihilus))])
        .expect("inventory")
        .grid(&tab(0), vec![(at(0, 0), object(&torch))])
        .expect("shared tab")
        .build();

    let plan = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(0),
            fingerprint: object(&torch).fingerprint(),
        },
        Destination::Grid {
            container: inventory.clone(),
            cell: Some(at(1, 0)),
        },
    )]);

    let outcome = accept(plan, vault);
    assert_eq!(
        outcome.grid(&inventory).expect("inventory").slots().len(),
        2
    );
}

#[test]
fn a_plan_names_a_container_the_vault_holds() {
    let vault = builder().grid(&tab(0), vec![]).expect("tab").build();
    let plan = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(3),
            fingerprint: object(&compact("r08")).fingerprint(),
        },
        Destination::Grid {
            container: tab(0),
            cell: None,
        },
    )]);
    assert_eq!(
        refuse(plan, vault),
        RuleViolation::UnknownContainer {
            container: ContainerId::Grid(tab(3)),
        }
    );
}

#[test]
fn a_plan_names_an_item_the_source_container_holds() {
    let vault = builder()
        .grid(&tab(0), vec![])
        .expect("target")
        .grid(&tab(1), vec![])
        .expect("source")
        .build();
    let fingerprint = object(&compact("r08")).fingerprint();
    let plan = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(1),
            fingerprint,
        },
        Destination::Grid {
            container: tab(0),
            cell: None,
        },
    )]);
    assert_eq!(
        refuse(plan, vault),
        RuleViolation::ObjectAbsent {
            container: ContainerId::Grid(tab(1)),
            fingerprint,
        }
    );
}

#[test]
fn a_grid_and_a_pile_never_share_an_identity() {
    let vault = builder()
        .grid(&tab(0), vec![(at(0, 0), object(&compact("r08")))])
        .expect("grid")
        .pile(&pile_tab(5), vec![])
        .expect("pile")
        .build();

    let pile_of_tab_zero = PileId::new(stash_name("shared"), TabIndex::new(0));
    let plan = Plan::new(vec![Operation::moved(
        Source::Pile {
            container: pile_of_tab_zero.clone(),
            fingerprint: object(&compact("r08")).fingerprint(),
            units: units(1),
        },
        Destination::Pile {
            container: pile_tab(5),
        },
    )]);
    assert_eq!(
        refuse(plan, vault),
        RuleViolation::UnknownContainer {
            container: ContainerId::Pile(pile_of_tab_zero),
        }
    );
}

#[test]
fn a_move_keeps_the_multiset_and_a_duplication_grows_it() {
    let rune = compact("r08");
    let vault = builder()
        .grid(&tab(0), vec![(at(0, 0), object(&rune))])
        .expect("source")
        .grid(&tab(1), vec![])
        .expect("target")
        .build();

    let moved = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(0),
            fingerprint: object(&rune).fingerprint(),
        },
        Destination::Grid {
            container: tab(1),
            cell: None,
        },
    )])
    .validate(vault.clone())
    .expect("the move is accepted");
    moved.conserved().expect("a move keeps the multiset");
    assert!(moved.duplicated().is_empty());
    assert_eq!(moved.realized()[0].transfer(), Transfer::Move);

    let duplicated = Plan::new(vec![Operation::duplicated(
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
    .expect("the duplication is accepted");
    duplicated
        .conserved()
        .expect("a duplication keeps the multiset it grew");
    assert_eq!(
        duplicated.duplicated(),
        &[(object(&rune).fingerprint(), 1)]
            .into_iter()
            .collect::<domain::census::Units>()
    );
    assert_eq!(duplicated.realized()[0].transfer(), Transfer::Duplicate);
    assert_eq!(
        duplicated
            .outcome()
            .grid(&tab(0))
            .expect("source")
            .slots()
            .len(),
        1
    );
}

#[test]
fn a_validated_plan_applies_only_against_the_outcome_it_expects() {
    let rune = compact("r08");
    let vault = builder()
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
            cell: Some(at(0, 0)),
        },
    )])
    .validate(vault.clone())
    .expect("the plan is accepted");

    let written = validated.outcome().clone();
    let applied = validated.apply(&written).expect("the outcome matches");
    assert!(applied.duplicated().is_empty());
    assert_eq!(applied.realized().len(), 1);

    let validated = Plan::new(vec![Operation::moved(
        Source::Grid {
            container: tab(0),
            fingerprint: object(&rune).fingerprint(),
        },
        Destination::Grid {
            container: tab(1),
            cell: Some(at(0, 0)),
        },
    )])
    .validate(vault.clone())
    .expect("the plan is accepted");
    assert_eq!(
        validated.apply(&vault),
        Err(ApplyError::UnitsDiffer {
            container: ContainerId::Grid(tab(0)),
            fingerprint: object(&rune).fingerprint(),
            expected: 0,
            observed: 1,
        })
    );
}

#[test]
fn a_fingerprint_reads_back_from_its_own_text() {
    let fingerprint = object(&compact("r08")).fingerprint();
    let text = fingerprint.to_string();
    assert_eq!(text.len(), 32);
    assert_eq!(domain::object::Fingerprint::parse(&text), Ok(fingerprint));
    assert_eq!(
        domain::object::Fingerprint::parse("dead"),
        Err(ValueError::FingerprintLength {
            length: 4,
            expected: 32,
        })
    );
    assert_eq!(
        domain::object::Fingerprint::parse(&"z".repeat(32)),
        Err(ValueError::FingerprintDigit { found: 'z' })
    );
}

#[test]
fn a_fingerprint_ignores_the_position_and_the_pile_count() {
    let rune = compact("r08");
    let moved = place(&rune, 7, 3);
    let piled = support::stacked(&rune, 42);
    let reference = object(&rune).fingerprint();
    assert_eq!(object(&moved).fingerprint(), reference);
    assert_eq!(object(&piled).fingerprint(), reference);
    assert_ne!(object(&compact("r09")).fingerprint(), reference);
    assert_ne!(object(&compact_marked("r08", 1)).fingerprint(), reference);
}
