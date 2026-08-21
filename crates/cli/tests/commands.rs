mod support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use clap::CommandFactory;
use clap::Parser;

use app::transaction::PlanDocument;
use cli::options::Options;

use support::{TempTree, copy_fixture, git, repository};

const ABSENT_TIMER: &str = "d2r-manager-tests-no-such.timer";
const CONTRACT: &str = include_str!("../../../docs/openapi.json");

fn fingerprints(directory: &Path) -> Vec<(String, u64)> {
    let mut prints: Vec<(String, u64)> = std::fs::read_dir(directory)
        .expect("the saves list")
        .map(|entry| {
            let entry = entry.expect("directory entry");
            let bytes = std::fs::read(entry.path()).expect("save file");
            let mut sum = 0u64;
            for byte in bytes {
                sum = sum.wrapping_mul(31).wrapping_add(u64::from(byte));
            }
            (entry.file_name().to_string_lossy().into_owned(), sum)
        })
        .collect();
    prints.sort();
    prints
}

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

fn listing(saves: &Path, container: &str) -> String {
    spoken(&[
        "d2r-manager",
        "ls",
        "--saves",
        &saved(saves),
        "--container",
        container,
    ])
}

fn fingerprint_of(saves: &Path, container: &str) -> String {
    listing(saves, container)
        .lines()
        .next()
        .expect("the container holds an item")
        .split_whitespace()
        .last()
        .expect("the fingerprint closes the line")
        .to_owned()
}

#[test]
fn the_command_tree_is_well_formed() {
    Options::command().debug_assert();
}

#[test]
fn the_api_contract_serves_every_task_of_the_command_line_and_no_other() {
    let contract: serde_json::Value =
        serde_json::from_str(CONTRACT).expect("the contract reads as JSON");
    let served: BTreeSet<String> = contract["paths"]
        .as_object()
        .expect("the paths of the contract")
        .values()
        .flat_map(|operations| {
            operations
                .as_object()
                .expect("the operations of a path")
                .values()
                .flat_map(|operation| {
                    operation["x-cli"]
                        .as_array()
                        .expect("the tasks an operation serves")
                        .iter()
                        .map(|task| task.as_str().expect("a task name").to_owned())
                })
        })
        .collect();
    let asked: BTreeSet<String> = Options::command()
        .get_subcommands()
        .map(|task| task.get_name().to_owned())
        .filter(|task| task != "help" && task != "serve")
        .collect();

    assert_eq!(served, asked);
}

#[test]
fn the_help_names_every_task_the_lot_delivers() {
    let help = Options::command().render_long_help().to_string();
    for task in ["sync", "ls", "find", "diff", "plan", "apply", "serve"] {
        assert!(help.contains(task), "{task} is missing from the help");
    }
    assert!(help.contains("d2r-manager"));
}

#[test]
fn the_serve_task_takes_a_port_and_never_a_host() {
    let mut serve = Options::command()
        .find_subcommand("serve")
        .cloned()
        .expect("the serve task");
    let arguments: Vec<String> = serve
        .get_arguments()
        .map(|argument| argument.get_id().to_string())
        .collect();

    assert!(arguments.contains(&"port".to_owned()), "{arguments:?}");
    assert!(
        arguments
            .iter()
            .all(|argument| !["host", "address", "bind", "interface"].contains(&argument.as_str())),
        "{arguments:?}"
    );
    assert!(
        serve.render_long_help().to_string().contains("127.0.0.1"),
        "the help of serve names no address"
    );
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
    let fingerprint = fingerprint_of(&saves, "shared/linux/0");

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
fn a_plan_leaves_every_save_byte_where_it_found_it() {
    let tree = TempTree::new("plan-writes-nothing");
    let saves = saves(&tree);
    let before = fingerprints(&saves);
    let fingerprint = fingerprint_of(&saves, "shared/linux/0");

    spoken(&[
        "d2r-manager",
        "plan",
        "--saves",
        &saved(&saves),
        "--operation",
        &format!("move:shared/linux/0#{fingerprint}:shared/linux/1"),
    ]);
    assert_eq!(fingerprints(&saves), before);
}

#[test]
fn the_sync_command_mirrors_and_commits_through_its_real_adapters() {
    let tree = TempTree::new("sync-command");
    let source = saves(&tree);
    let repo = repository(&tree, "backup");
    let text = spoken(&[
        "d2r-manager",
        "sync",
        "--source",
        &saved(&source),
        "--repo",
        &saved(&repo),
        "--lock",
        &saved(&tree.path("lock/backup.lock")),
    ]);
    assert!(text.starts_with("sync: committed "), "{text}");
    assert!(repo.join("saves/linux.d2i").is_file());
    assert!(
        git(&repo, &["log", "-1", "--format=%s"]).starts_with("backup: "),
        "the sync command committed nothing"
    );
}

#[test]
fn the_apply_command_runs_a_plan_file_end_to_end() {
    let tree = TempTree::new("apply-command");
    let repo = repository(&tree, "backup");
    let saves = repo.join("saves");
    std::fs::create_dir_all(&saves).expect("saves directory");
    copy_fixture(
        "v105/linux/ModernSharedStashSoftCoreV2.d2i",
        &saves.join("linux.d2i"),
    );
    git(&repo, &["add", "-A", "--", "saves"]);
    git(&repo, &["commit", "-m", "saves"]);

    let fingerprint = fingerprint_of(&saves, "shared/linux/0");
    let file = tree.path("plan.json");
    std::fs::write(
        &file,
        spoken(&[
            "d2r-manager",
            "plan",
            "--saves",
            &saved(&saves),
            "--operation",
            &format!("move:shared/linux/0#{fingerprint}:shared/linux/2"),
        ]),
    )
    .expect("the plan file");

    let text = spoken(&[
        "d2r-manager",
        "apply",
        "--repo",
        &saved(&repo),
        "--plan",
        &saved(&file),
        "--timer",
        ABSENT_TIMER,
    ]);

    assert!(text.contains("apply: committed"), "{text}");
    assert!(text.contains(&fingerprint), "{text}");
    assert!(listing(&saves, "shared/linux/2").contains(&fingerprint));
    assert!(!listing(&saves, "shared/linux/0").contains(&fingerprint));
    assert!(
        git(&repo, &["log", "-1", "--format=%s"]).starts_with("apply: "),
        "the binary committed nothing"
    );
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
