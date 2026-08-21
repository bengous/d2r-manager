mod support;

use std::path::{Path, PathBuf};

use clap::CommandFactory;
use clap::Parser;

use app::transaction::PlanDocument;
use cli::options::Options;

use support::{TempTree, copy_fixture};

fn saves(tree: &TempTree) -> PathBuf {
    let saves = tree.directory("saves");
    copy_fixture(
        "v105/linux/ModernSharedStashSoftCoreV2.d2i",
        &saves.join("linux.d2i"),
    );
    copy_fixture("v105/linux/Gous.d2s", &saves.join("Gous.d2s"));
    saves
}

fn spoken(arguments: &[&str]) -> String {
    let options = Options::try_parse_from(arguments).expect("the arguments parse");
    let mut out = Vec::new();
    cli::run(&options, &mut out).expect("the command runs");
    String::from_utf8(out).expect("utf-8")
}

fn saved(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn the_command_tree_is_well_formed() {
    Options::command().debug_assert();
}

#[test]
fn the_help_names_every_task_the_lot_delivers() {
    let help = Options::command().render_long_help().to_string();
    for task in ["sync", "ls", "find", "diff", "plan", "apply"] {
        assert!(help.contains(task), "{task} is missing from the help");
    }
    assert!(help.contains("d2r-manager"));
}

#[test]
fn the_listing_prints_one_line_for_every_item() {
    let tree = TempTree::new("ls");
    let saves = saves(&tree);
    let text = spoken(&["d2r-manager", "ls", "--saves", &saved(&saves)]);
    assert_eq!(text.lines().count(), 262);
    assert!(
        text.lines()
            .all(|line| line.starts_with("shared/linux/") || line.starts_with("char/Gous/"))
    );
}

#[test]
fn the_listing_of_one_container_prints_that_container_alone() {
    let tree = TempTree::new("ls-one");
    let saves = saves(&tree);
    let text = spoken(&[
        "d2r-manager",
        "ls",
        "--saves",
        &saved(&saves),
        "--container",
        "char/Gous/inventory",
    ]);
    assert!(!text.is_empty());
    assert!(
        text.lines()
            .all(|line| line.starts_with("char/Gous/inventory"))
    );
}

#[test]
fn a_search_by_code_prints_that_code_alone() {
    let tree = TempTree::new("find");
    let saves = saves(&tree);
    let text = spoken(&[
        "d2r-manager",
        "find",
        "--saves",
        &saved(&saves),
        "--code",
        "cm1",
        "--quality",
        "magic",
    ]);
    assert!(!text.is_empty());
    assert!(
        text.lines()
            .all(|line| line.contains("cm1") && line.contains("magic"))
    );
}

#[test]
fn a_comparison_of_a_directory_with_itself_prints_nothing() {
    let tree = TempTree::new("diff");
    let saves = saves(&tree);
    let text = spoken(&[
        "d2r-manager",
        "diff",
        "--saves",
        &saved(&saves),
        "--against",
        &saved(&saves),
    ]);
    assert!(text.is_empty(), "{text}");
}

#[test]
fn a_plan_prints_json_that_reads_back_as_a_plan() {
    let tree = TempTree::new("plan");
    let saves = saves(&tree);
    let listing = spoken(&[
        "d2r-manager",
        "ls",
        "--saves",
        &saved(&saves),
        "--container",
        "shared/linux/0",
    ]);
    let fingerprint = listing
        .lines()
        .next()
        .expect("the tab holds an item")
        .split_whitespace()
        .last()
        .expect("the fingerprint closes the line")
        .to_owned();

    let text = spoken(&[
        "d2r-manager",
        "plan",
        "--saves",
        &saved(&saves),
        "--operation",
        &format!("move:shared/linux/0#{fingerprint}:shared/linux/1"),
    ]);
    let document: PlanDocument = serde_json::from_str(&text).expect("the plan reads back");
    assert_eq!(document.operations.len(), 1);
    assert_eq!(document.operations[0].from.fingerprint, fingerprint);
    assert_eq!(document.operations[0].to.container, "shared/linux/1");
}

#[test]
fn a_plan_the_rules_refuse_prints_nothing() {
    let tree = TempTree::new("plan-refused");
    let saves = saves(&tree);
    let options = Options::try_parse_from([
        "d2r-manager",
        "plan",
        "--saves",
        &saved(&saves),
        "--operation",
        "move:shared/linux/0#00000000000000000000000000000000:shared/linux/1",
    ])
    .expect("the arguments parse");
    let mut out = Vec::new();
    let error = cli::run(&options, &mut out).expect_err("no item carries that fingerprint");
    assert!(out.is_empty());
    assert!(format!("{error:#}").contains("fingerprint"), "{error:#}");
}
