use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const TXT_DIR: &str = "reference/d2rsavegameparser/txt";
const GENERATED: &str = "crates/domain/src/tables/v105.rs";
const ITEM_FILES: [&str; 3] = ["weapons.txt", "armor.txt", "misc.txt"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

#[expect(
    clippy::disallowed_methods,
    reason = "PLAN 6.5: the table guard reads the reference txt files"
)]
fn read_txt(name: &str) -> String {
    let path = repo_root().join(TXT_DIR).join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

fn rows(content: &str) -> (HashMap<String, usize>, Vec<String>) {
    let mut lines = content.split('\n').filter(|line| !line.is_empty());
    let header_line = lines.next().unwrap_or_default();
    let mut header = HashMap::new();
    for (index, name) in header_line.split('\t').enumerate() {
        let name = name.trim().to_lowercase();
        if !name.is_empty() {
            header.entry(name).or_insert(index);
        }
    }
    let body = lines
        .map(|line| line.trim_end_matches('\r').to_owned())
        .collect();
    (header, body)
}

fn column<'a>(columns: &'a [&'a str], index: usize) -> &'a str {
    columns.get(index).map_or("", |value| value.trim())
}

fn required(header: &HashMap<String, usize>, name: &str) -> usize {
    header
        .get(name)
        .copied()
        .unwrap_or_else(|| panic!("column {name} is missing from the reference txt file"))
}

fn make_code(text: &str) -> u32 {
    assert!(text.len() <= 4, "item code {text} is longer than 4 bytes");
    let mut bytes = [b' '; 4];
    for (slot, byte) in bytes.iter_mut().zip(text.bytes()) {
        *slot = byte;
    }
    u32::from_le_bytes(bytes)
}

fn parse_footprints() -> Vec<(u32, u8, u8)> {
    let mut seen: Vec<u32> = Vec::new();
    let mut entries: Vec<(u32, u8, u8)> = Vec::new();
    for file in ITEM_FILES {
        let content = read_txt(file);
        let (header, body) = rows(&content);
        let (code, width, height) = (
            required(&header, "code"),
            required(&header, "invwidth"),
            required(&header, "invheight"),
        );
        for line in &body {
            let columns: Vec<&str> = line.split('\t').collect();
            let text = column(&columns, code);
            if text.is_empty() {
                continue;
            }
            let key = make_code(text);
            if seen.contains(&key) {
                continue;
            }
            let width = column(&columns, width)
                .parse::<u8>()
                .unwrap_or_else(|error| panic!("{file}: item {text} width: {error}"));
            let height = column(&columns, height)
                .parse::<u8>()
                .unwrap_or_else(|error| panic!("{file}: item {text} height: {error}"));
            assert!(
                (1..=16).contains(&width) && (1..=16).contains(&height),
                "{file}: item {text} measures {width}x{height}, outside 1..=16"
            );
            seen.push(key);
            entries.push((key, width, height));
        }
    }
    entries.sort_unstable();
    entries
}

fn parse_carry_groups() -> Vec<(u16, u16)> {
    let content = read_txt("uniqueitems.txt");
    let (header, body) = rows(&content);
    let (id, carry) = (required(&header, "*id"), required(&header, "carry1"));

    let mut entries: Vec<(u16, u16)> = Vec::new();
    for line in &body {
        let columns: Vec<&str> = line.split('\t').collect();
        let group = column(&columns, carry);
        if group.is_empty() {
            continue;
        }
        let unique = column(&columns, id)
            .parse::<u16>()
            .unwrap_or_else(|error| panic!("uniqueitems.txt: unique id: {error}"));
        let group = group
            .parse::<u16>()
            .unwrap_or_else(|error| panic!("uniqueitems.txt: carry1 group: {error}"));
        entries.push((unique, group));
    }
    entries.sort_unstable();
    entries
}

fn render() -> String {
    let footprints = parse_footprints();
    let groups = parse_carry_groups();

    let mut out = String::new();
    out.push_str("use super::{CarryGroup, Footprint};\n\n#[rustfmt::skip]\npub(super) static FOOTPRINTS: [(u32, Footprint); ");
    writeln!(out, "{}] = [", footprints.len()).unwrap();
    for (code, width, height) in &footprints {
        let text = String::from_utf8(code.to_le_bytes().to_vec()).expect("ascii item code");
        writeln!(
            out,
            "    (u32::from_le_bytes(*b\"{text}\"), Footprint::new({width}, {height})),"
        )
        .unwrap();
    }
    out.push_str("];\n\n#[rustfmt::skip]\npub(super) static CARRY_GROUPS: [(u16, CarryGroup); ");
    writeln!(out, "{}] = [", groups.len()).unwrap();
    for (unique, group) in &groups {
        writeln!(out, "    ({unique}, CarryGroup::new({group})),").unwrap();
    }
    out.push_str("];\n");
    out
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "PLAN 6.5: the table guard reads and rewrites the generated table"
)]
fn the_version_105_tables_match_the_reference_txt_files() {
    let path = repo_root().join(GENERATED);
    let rendered = render();

    if std::env::var_os("UPDATE_TABLES").is_some() {
        std::fs::write(&path, &rendered).expect("write generated tables");
        return;
    }

    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == rendered,
        "{} is stale, regenerate it with `just tables`",
        path.display()
    );
}

#[test]
fn the_named_exclusive_charms_keep_their_unique_identifier() {
    let content = read_txt("uniqueitems.txt");
    let (header, body) = rows(&content);
    let (index, id, code) = (
        required(&header, "index"),
        required(&header, "*id"),
        required(&header, "code"),
    );

    let mut found: Vec<(String, u16, String)> = Vec::new();
    for line in &body {
        let columns: Vec<&str> = line.split('\t').collect();
        let name = column(&columns, index);
        if !matches!(name, "Annihilus" | "Hellfire Torch" | "Gheed's Fortune") {
            continue;
        }
        found.push((
            name.to_owned(),
            column(&columns, id).parse::<u16>().expect("unique id"),
            column(&columns, code).to_owned(),
        ));
    }
    found.sort_unstable();

    assert_eq!(
        found,
        vec![
            ("Annihilus".to_owned(), 381, "cm1".to_owned()),
            ("Gheed's Fortune".to_owned(), 359, "cm3".to_owned()),
            ("Hellfire Torch".to_owned(), 400, "cm2".to_owned()),
        ]
    );
}
