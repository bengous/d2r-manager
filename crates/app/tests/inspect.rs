mod support;

use std::path::Path;
use std::str::FromStr;

use app::inspect::{FingerprintPrefix, Needle, Placement, Quality, Row, compare, list};
use app::selector::{ContainerSelector, SelectorError};
use app::shorthand::{ShorthandError, parse};
use app::transaction::{
    CellDocument, DestinationDocument, OperationDocument, PlanDocument, SourceDocument,
    TransferDocument, describe, resolve,
};
use app::world::SaveTree;
use domain::container::{CharacterPage, StashName, TabIndex};
use domain::object::Fingerprint;
use format::code::ItemCode;

use support::{FakeFiles, fixture};

const SAVES: &str = "/saves";
const STASH: &str = "linux";

fn files() -> FakeFiles {
    let files = FakeFiles::new();
    files.directory(SAVES);
    files.file(
        Path::new(SAVES).join(format!("{STASH}.d2i")),
        fixture("v105/linux/ModernSharedStashSoftCoreV2.d2i"),
    );
    files.file(
        Path::new(SAVES).join("Gous.d2s"),
        fixture("v105/linux/Gous.d2s"),
    );
    files
}

fn vault() -> domain::vault::Vault {
    SaveTree::read(&files(), Path::new(SAVES))
        .expect("the saves read")
        .vault()
        .expect("the vault builds")
}

fn order(row: &Row) -> (ContainerSelector, u8, u8, u8, Fingerprint) {
    let (line, column, units) = match row.placement {
        Placement::Cell(cell) => (cell.row().get(), cell.column().get(), 0),
        Placement::Units(count) => (0, 0, count.get()),
    };
    (row.container.clone(), line, column, units, row.fingerprint)
}

#[test]
fn the_listing_comes_out_container_by_container_in_reading_order() {
    let vault = vault();
    let rows = list(&vault, &Needle::default());
    assert_eq!(rows.len(), 262);

    let keys: Vec<_> = rows.iter().map(order).collect();
    let out_of_order: Vec<_> = keys
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0] > pair[1])
        .map(|(index, _)| index)
        .collect();
    assert!(
        out_of_order.is_empty(),
        "rows out of order at {out_of_order:?}"
    );

    let head = rows.first().expect("the saves hold items");
    assert_eq!(head.container.to_string(), format!("shared/{STASH}/0"));
    assert_eq!(head.placement.to_string(), "0,0");
    assert_eq!(
        head.fingerprint.to_string(),
        "5219cb83a1c0bbc9a27d80235f963d24"
    );
    assert_eq!(rows, list(&vault, &Needle::default()));
}

#[test]
fn a_container_filter_keeps_that_container_alone() {
    let vault = vault();
    let container = ContainerSelector::from_str(&format!("shared/{STASH}/5")).expect("selector");
    let rows = list(
        &vault,
        &Needle {
            container: Some(container.clone()),
            ..Needle::default()
        },
    );
    assert_eq!(rows.len(), 81);
    assert!(rows.iter().all(|row| row.container == container));
    assert!(
        rows.iter()
            .all(|row| matches!(row.placement, Placement::Units(_)))
    );
}

#[test]
fn a_code_filter_keeps_that_code_alone() {
    let vault = vault();
    let code = ItemCode::from_text("cm1").expect("item code");
    let rows = list(
        &vault,
        &Needle {
            code: Some(code),
            ..Needle::default()
        },
    );
    assert_ne!(rows, []);
    assert!(rows.iter().all(|row| row.code == Some(code)));
}

#[test]
fn a_quality_filter_keeps_that_quality_alone() {
    let vault = vault();
    let rows = list(
        &vault,
        &Needle {
            quality: Some(Quality::Unique),
            ..Needle::default()
        },
    );
    assert_ne!(rows, []);
    assert!(rows.iter().all(|row| row.quality == Quality::Unique));
}

#[test]
fn a_fingerprint_prefix_keeps_the_items_it_covers() {
    let vault = vault();
    let all = list(&vault, &Needle::default());
    let wanted = all.first().expect("the saves hold items").fingerprint;
    let prefix = FingerprintPrefix::new(&wanted.to_string()[..8]).expect("prefix");
    let rows = list(
        &vault,
        &Needle {
            fingerprint: Some(prefix),
            ..Needle::default()
        },
    );
    assert_ne!(rows, []);
    assert!(rows.iter().all(|row| {
        row.fingerprint
            .to_string()
            .starts_with(&wanted.to_string()[..8])
    }));
}

#[test]
fn a_fingerprint_prefix_outside_hexadecimal_is_refused() {
    assert!(FingerprintPrefix::new("zz").is_err());
    assert!(FingerprintPrefix::new("").is_err());
}

#[test]
fn a_move_shows_up_as_one_loss_and_one_gain_in_the_comparison() {
    let files = files();
    let mut tree = SaveTree::read(&files, Path::new(SAVES)).expect("the saves read");
    let before = tree.vault().expect("the vault builds");
    let fingerprint = before
        .grid(
            &ContainerSelector::from_str(&format!("shared/{STASH}/0"))
                .expect("selector")
                .grid(),
        )
        .expect("tab 0")
        .slots()
        .first()
        .expect("the tab holds an item")
        .object()
        .fingerprint();

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
                cell: None,
            },
        }],
    };
    let plan = resolve(&document, &before).expect("the plan resolves");
    let validated = plan.validate(before).expect("the plan is valid");
    tree.project(validated.outcome())
        .expect("the world projects");
    tree.flush(&files).expect("the saves write");
    let after = SaveTree::read(&files, Path::new(SAVES))
        .expect("the saves read back")
        .vault()
        .expect("the vault builds again");

    let before = vault();
    let differences = compare(&before, &after);
    assert_eq!(differences.len(), 2);
    assert!(differences.iter().all(|row| row.fingerprint == fingerprint));
    assert_eq!(
        differences[0].container.to_string(),
        format!("shared/{STASH}/0")
    );
    assert_eq!(differences[0].before, 1);
    assert_eq!(differences[0].after, 0);
    assert_eq!(
        differences[1].container.to_string(),
        format!("shared/{STASH}/1")
    );
    assert_eq!(differences[1].before, 0);
    assert_eq!(differences[1].after, 1);
}

#[test]
fn a_comparison_of_a_world_with_itself_is_empty() {
    assert_eq!(compare(&vault(), &vault()), []);
}

#[test]
fn a_container_reads_back_from_the_text_it_prints() {
    let selectors = [
        ContainerSelector::SharedTab {
            stash: StashName::new("linux").expect("stash"),
            tab: TabIndex::new(5),
        },
        ContainerSelector::CharacterPage {
            character: domain::container::CharacterName::new("Gous").expect("character"),
            page: CharacterPage::Cube,
        },
    ];
    for selector in selectors {
        let text = selector.to_string();
        assert_eq!(
            ContainerSelector::from_str(&text).expect("the text reads back"),
            selector
        );
    }
}

#[test]
fn a_container_outside_the_grammar_is_refused() {
    assert!(matches!(
        ContainerSelector::from_str("shared/linux").expect_err("two parts is not a container"),
        SelectorError::Shape { .. }
    ));
    assert!(matches!(
        ContainerSelector::from_str("stash/linux/0").expect_err("stash names no family"),
        SelectorError::Family { .. }
    ));
    assert!(matches!(
        ContainerSelector::from_str("char/Gous/belt").expect_err("belt is no page"),
        SelectorError::Page { .. }
    ));
    assert!(matches!(
        ContainerSelector::from_str("shared/linux/x").expect_err("x is no tab"),
        SelectorError::Tab { .. }
    ));
}

#[test]
fn the_shorthand_reads_the_three_shapes_a_plan_takes() {
    let print = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
    assert_eq!(
        parse(&format!("move:shared/linux/3#{print}:shared/linux/4@0,2")).expect("shorthand"),
        OperationDocument {
            transfer: TransferDocument::Move,
            from: SourceDocument {
                container: "shared/linux/3".to_owned(),
                fingerprint: print.to_owned(),
                units: None,
            },
            to: DestinationDocument {
                container: "shared/linux/4".to_owned(),
                cell: Some(CellDocument { column: 0, row: 2 }),
            },
        }
    );
    assert_eq!(
        parse(&format!("move:shared/linux/5#{print}*20:shared/linux/5"))
            .expect("shorthand")
            .from
            .units,
        Some(20)
    );
    assert_eq!(
        parse(&format!(
            "duplicate:char/Gous/stash#{print}:char/Elektra/cube"
        ))
        .expect("shorthand")
        .transfer,
        TransferDocument::Duplicate
    );
}

#[test]
fn the_shorthand_refuses_what_it_cannot_read() {
    let print = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
    assert!(matches!(
        parse("move:shared/linux/3").expect_err("two parts is no operation"),
        ShorthandError::Shape { .. }
    ));
    assert!(matches!(
        parse(&format!("swap:shared/linux/3#{print}:shared/linux/4"))
            .expect_err("swap is no transfer"),
        ShorthandError::Transfer { .. }
    ));
    assert!(matches!(
        parse("move:shared/linux/3:shared/linux/4").expect_err("a source names an item"),
        ShorthandError::Source { .. }
    ));
    assert!(matches!(
        parse(&format!("move:shared/linux/5#{print}*x:shared/linux/5"))
            .expect_err("x is no unit count"),
        ShorthandError::Units { .. }
    ));
    assert!(matches!(
        parse(&format!("move:shared/linux/3#{print}:shared/linux/4@0"))
            .expect_err("a cell names two numbers"),
        ShorthandError::Cell { .. }
    ));
}

#[test]
fn a_plan_prints_the_operations_it_was_given() {
    let vault = vault();
    let all = list(&vault, &Needle::default());
    let row = all.first().expect("the saves hold items");
    let document = PlanDocument {
        operations: vec![OperationDocument {
            transfer: TransferDocument::Move,
            from: SourceDocument {
                container: row.container.to_string(),
                fingerprint: row.fingerprint.to_string(),
                units: None,
            },
            to: DestinationDocument {
                container: format!("shared/{STASH}/4"),
                cell: Some(CellDocument { column: 0, row: 0 }),
            },
        }],
    };
    let plan = resolve(&document, &vault).expect("the plan resolves");
    assert_eq!(describe(plan.operations()), document);
}
