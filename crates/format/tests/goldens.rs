mod corpus;
mod project;

use corpus::{fixture_bytes, fixture_names, is_stash, repo_root};
use format::save::CharacterSave;
use format::stash::StashSave;
use serde_json::{Map, Value};

const LINUX: &str = "v105/linux";

fn golden(name: &str) -> Value {
    let path = repo_root()
        .join("goldens/csharp")
        .join(LINUX)
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).expect("golden json")
}

fn keys_without_type(map: &Map<String, Value>) -> Vec<&str> {
    let mut keys: Vec<&str> = map
        .keys()
        .filter(|key| key.as_str() != "$type")
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

fn is_value_only(map: &Map<String, Value>) -> bool {
    map.len() == 1 && map.contains_key("value")
}

fn compare(mine: &Value, golden: &Value, path: &str, failures: &mut Vec<String>) {
    match (mine, golden) {
        (Value::Object(left), Value::Object(right)) => {
            if is_value_only(left) && right.contains_key("name") && right.contains_key("value") {
                compare(&left["value"], &right["value"], path, failures);
                return;
            }
            let left_keys = keys_without_type(left);
            let right_keys = keys_without_type(right);
            if left_keys != right_keys {
                failures.push(format!(
                    "{path}: keys {left_keys:?} against golden {right_keys:?}"
                ));
                return;
            }
            for key in left_keys {
                compare(&left[key], &right[key], &format!("{path}.{key}"), failures);
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                failures.push(format!(
                    "{path}: {} entries against golden {}",
                    left.len(),
                    right.len()
                ));
                return;
            }
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                compare(left, right, &format!("{path}[{index}]"), failures);
            }
        }
        _ => {
            if mine != golden {
                failures.push(format!("{path}: {mine} against golden {golden}"));
            }
        }
    }
}

#[test]
fn every_version_105_linux_fixture_matches_its_csharp_golden_field_by_field() {
    let names = fixture_names(LINUX);
    assert_eq!(names.len(), 55, "corpus size");

    let mut failures = Vec::new();
    for name in &names {
        let data = fixture_bytes(LINUX, name);
        let golden = golden(name);
        let mine = if is_stash(name) {
            let save = StashSave::read(&data).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(
                save.trailing.is_empty(),
                "{name}: {} bytes past the last tab",
                save.trailing.len()
            );
            project::stash_save(&save)
        } else {
            let save = CharacterSave::read(&data).unwrap_or_else(|error| panic!("{name}: {error}"));
            project::character_save(&save)
        };
        compare(&mine, &golden["model"], name, &mut failures);
    }

    assert!(
        failures.is_empty(),
        "{} field difference(s):\n{}",
        failures.len(),
        failures
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_anti_regression_markers_of_the_modern_shared_stash_hold() {
    let name = "ModernSharedStashSoftCoreV2.d2i";
    let save = StashSave::read(&fixture_bytes(LINUX, name)).expect("stash");

    assert_eq!(save.tabs.len(), 7, "tab count");
    let kinds: Vec<_> = save
        .tabs
        .iter()
        .map(format::stash::StashTab::kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            format::stash::TabKind::Normal,
            format::stash::TabKind::Normal,
            format::stash::TabKind::Normal,
            format::stash::TabKind::Normal,
            format::stash::TabKind::Normal,
            format::stash::TabKind::AdvancedStash,
            format::stash::TabKind::Chronicle,
        ]
    );
    assert_eq!(save.tabs[0].gold, 1_075_411, "gold in the first tab");

    let mut entries = 0usize;
    let mut stacks = 0u32;
    for tab in &save.tabs {
        let Some(items) = tab.body.items() else {
            continue;
        };
        entries += items.0.len();
        if tab.kind() == format::stash::TabKind::AdvancedStash {
            for item in &items.0 {
                stacks += advanced_stash_stack(item);
            }
        }
    }
    assert_eq!(entries, 227, "item entries");
    assert_eq!(stacks, 1306, "advanced stash stacked units");

    let chronicle_tab = format::stash::TAB_HEADER_BYTES
        + chronicle_body_bytes(&save.tabs[6])
        + save.tabs[6].trailing.len();
    assert_eq!(chronicle_tab, 2298, "chronicle tab, header included");
}

fn advanced_stash_stack(item: &format::item::Item) -> u32 {
    let stack = match &item.body {
        format::item::ItemBody::Compact(body) => body.advanced_stash_stack,
        format::item::ItemBody::Complete(body) => body.advanced_stash_stack,
    };
    stack.map_or(0, format::bits::Bits::get)
}

fn chronicle_body_bytes(tab: &format::stash::StashTab) -> usize {
    let format::stash::TabBody::Chronicle(section) = &tab.body else {
        panic!("tab 6 is not a chronicle tab");
    };
    20 + (section.sets.len() + section.uniques.len() + section.runewords.len()) * 10
}
