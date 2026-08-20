use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const TXT_DIR: &str = "reference/D2SSharp/src/D2SSharp/Data/Txt/105";
const GENERATED: &str = "crates/format/src/tables/v105.rs";

const COMPACT_SAVE: u16 = 1 << 0;
const VARIABLE_GFX: u16 = 1 << 1;
const ARMOR: u16 = 1 << 2;
const WEAPON: u16 = 1 << 3;
const GOLD: u16 = 1 << 4;
const CHARM: u16 = 1 << 5;
const BODY_PART: u16 = 1 << 6;
const PLAYER_BODY_PART: u16 = 1 << 7;
const SCROLL_OR_BOOK: u16 = 1 << 8;
const QUEST: u16 = 1 << 9;
const QUEST_DIFF_CHECK: u16 = 1 << 10;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn read_txt(name: &str) -> String {
    let path = repo_root().join(TXT_DIR).join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
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

fn column<'a>(columns: &'a [&'a str], index: Option<usize>) -> &'a str {
    index
        .and_then(|index| columns.get(index))
        .map_or("", |value| value.trim())
}

fn parse_u32(value: &str) -> u32 {
    value.parse::<u8>().map_or(0, u32::from)
}

fn parse_i32(value: &str) -> i32 {
    value.parse::<i32>().unwrap_or(0)
}

fn parse_bool(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("true")
}

fn make_code(text: &str) -> u32 {
    assert!(text.len() <= 4, "item code {text} is longer than 4 bytes");
    let mut bytes = [b' '; 4];
    for (slot, byte) in bytes.iter_mut().zip(text.bytes()) {
        *slot = byte;
    }
    u32::from_le_bytes(bytes)
}

fn code_text(code: u32) -> String {
    String::from_utf8_lossy(&code.to_le_bytes())
        .trim_end_matches(['\0', ' '])
        .to_owned()
}

struct Stat {
    csv_bits: u32,
    csv_param: u32,
    csv_signed: bool,
    save_bits: u32,
    save_param_bits: u32,
    save_add: i32,
    val_shift: u32,
}

fn parse_stats() -> Vec<Stat> {
    let content = read_txt("itemstatcost.txt");
    let (header, body) = rows(&content);
    let get = |name: &str| header.get(name).copied();
    let (signed, bits, param) = (get("csvsigned"), get("csvbits"), get("csvparam"));
    let (save_bits, save_param, save_add, shift) = (
        get("save bits"),
        get("save param bits"),
        get("save add"),
        get("valshift"),
    );

    body.iter()
        .map(|line| {
            if line.trim().is_empty() {
                return Stat {
                    csv_bits: 0,
                    csv_param: 0,
                    csv_signed: false,
                    save_bits: 0,
                    save_param_bits: 0,
                    save_add: 0,
                    val_shift: 0,
                };
            }
            let columns: Vec<&str> = line.split('\t').collect();
            let save_param_bits = parse_i32(column(&columns, save_param));
            assert!(
                (0..=32).contains(&save_param_bits),
                "save param bits {save_param_bits} outside 0..=32"
            );
            Stat {
                csv_bits: parse_u32(column(&columns, bits)),
                csv_param: parse_u32(column(&columns, param)),
                csv_signed: parse_bool(column(&columns, signed)),
                save_bits: parse_u32(column(&columns, save_bits)),
                #[expect(clippy::cast_sign_loss, reason = "range asserted above")]
                save_param_bits: save_param_bits as u32,
                save_add: parse_i32(column(&columns, save_add)),
                val_shift: parse_u32(column(&columns, shift)),
            }
        })
        .collect()
}

#[derive(Default, Clone)]
struct ItemType {
    code: u32,
    equiv1_name: String,
    equiv2_name: String,
    equiv1: u16,
    equiv2: u16,
    var_inv_gfx: u32,
}

fn parse_item_types() -> Vec<ItemType> {
    let content = read_txt("itemtypes.txt");
    let (header, body) = rows(&content);
    let get = |name: &str| header.get(name).copied();
    let (code, equiv1, equiv2, gfx) = (get("code"), get("equiv1"), get("equiv2"), get("varinvgfx"));

    let mut types: Vec<ItemType> = Vec::new();
    let mut name_to_index: HashMap<String, usize> = HashMap::new();
    for line in &body {
        if line.trim().is_empty() {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        let code_text = column(&columns, code);
        if code_text.is_empty() {
            types.push(ItemType::default());
            continue;
        }
        let index = types.len();
        types.push(ItemType {
            code: make_code(code_text),
            equiv1_name: column(&columns, equiv1).to_owned(),
            equiv2_name: column(&columns, equiv2).to_owned(),
            equiv1: 0,
            equiv2: 0,
            var_inv_gfx: parse_u32(column(&columns, gfx)),
        });
        name_to_index
            .entry(code_text.to_lowercase())
            .or_insert(index);
    }

    let resolve = |name: &str| {
        if name.is_empty() {
            return 0xFFFF;
        }
        name_to_index
            .get(&name.to_lowercase())
            .and_then(|index| u16::try_from(*index).ok())
            .unwrap_or(0xFFFF)
    };
    for entry in &mut types {
        entry.equiv1 = resolve(&entry.equiv1_name);
        entry.equiv2 = resolve(&entry.equiv2_name);
    }
    types
}

struct Item {
    code: u32,
    type_index: usize,
    compact_save: bool,
    quest: bool,
    quest_diff_check: bool,
}

fn parse_items(file: &str, type_names: &HashMap<String, usize>) -> Vec<Item> {
    let content = read_txt(file);
    let (header, body) = rows(&content);
    let get = |name: &str| header.get(name).copied();
    let (code, kind, compact, quest, diff) = (
        get("code"),
        get("type"),
        get("compactsave"),
        get("quest"),
        get("questdiffcheck"),
    );

    body.iter()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let columns: Vec<&str> = line.split('\t').collect();
            let code_text = column(&columns, code);
            if code_text.is_empty() {
                return None;
            }
            let type_name = column(&columns, kind).to_lowercase();
            Some(Item {
                code: make_code(code_text),
                type_index: type_names.get(&type_name).copied().unwrap_or(0),
                compact_save: parse_u32(column(&columns, compact)) != 0,
                quest: parse_u32(column(&columns, quest)) != 0,
                quest_diff_check: parse_u32(column(&columns, diff)) != 0,
            })
        })
        .collect()
}

fn descendants(types: &[ItemType]) -> HashMap<u32, Vec<u32>> {
    fn collect(
        index: usize,
        types: &[ItemType],
        children: &[Vec<usize>],
        seen: &mut Vec<usize>,
        out: &mut Vec<u32>,
    ) {
        if seen.contains(&index) {
            return;
        }
        seen.push(index);
        if types[index].code != 0 && !out.contains(&types[index].code) {
            out.push(types[index].code);
        }
        for child in &children[index] {
            collect(*child, types, children, seen, out);
        }
    }

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); types.len()];
    for (index, entry) in types.iter().enumerate() {
        for parent in [entry.equiv1, entry.equiv2] {
            if parent != 0 && parent != 0xFFFF {
                children[usize::from(parent)].push(index);
            }
        }
    }

    let mut result: HashMap<u32, Vec<u32>> = HashMap::new();
    for (index, entry) in types.iter().enumerate() {
        if entry.code == 0 {
            continue;
        }
        let mut set = Vec::new();
        collect(index, types, &children, &mut Vec::new(), &mut set);
        let bucket = result.entry(entry.code).or_default();
        for code in set {
            if !bucket.contains(&code) {
                bucket.push(code);
            }
        }
    }
    result
}

fn render() -> String {
    let stats = parse_stats();
    let types = parse_item_types();

    let mut type_names: HashMap<String, usize> = HashMap::new();
    for (index, entry) in types.iter().enumerate() {
        if entry.code != 0 {
            type_names
                .entry(code_text(entry.code).to_lowercase())
                .or_insert(index);
        }
    }

    let mut items = parse_items("weapons.txt", &type_names);
    items.extend(parse_items("armor.txt", &type_names));
    items.extend(parse_items("misc.txt", &type_names));

    let tree = descendants(&types);
    let is_descendant = |root: &str, code: u32| {
        code != 0
            && tree
                .get(&make_code(root))
                .is_some_and(|set| set.contains(&code))
    };

    let mut seen: Vec<u32> = Vec::new();
    let mut entries: Vec<(u32, u16)> = Vec::new();
    for item in &items {
        if item.code == 0 || seen.contains(&item.code) {
            continue;
        }
        seen.push(item.code);

        let kind = types.get(item.type_index);
        let type_code = kind.map_or(0, |entry| entry.code);
        let mut bits = 0u16;
        for (flag, set) in [
            (COMPACT_SAVE, item.compact_save),
            (
                VARIABLE_GFX,
                kind.is_some_and(|entry| entry.var_inv_gfx > 0),
            ),
            (ARMOR, is_descendant("armo", type_code)),
            (WEAPON, is_descendant("weap", type_code)),
            (GOLD, is_descendant("gold", type_code)),
            (CHARM, is_descendant("char", type_code)),
            (BODY_PART, is_descendant("body", type_code)),
            (PLAYER_BODY_PART, is_descendant("play", type_code)),
            (
                SCROLL_OR_BOOK,
                is_descendant("scro", type_code) || is_descendant("book", type_code),
            ),
            (QUEST, item.quest),
            (QUEST_DIFF_CHECK, item.quest_diff_check),
        ] {
            if set {
                bits |= flag;
            }
        }
        entries.push((item.code, bits));
    }
    entries.sort_unstable();

    let mut out = String::new();
    out.push_str("use super::{ItemInfo, StatInfo};\n\n#[rustfmt::skip]\npub(super) static STATS: [StatInfo; ");
    writeln!(out, "{}] = [", stats.len()).unwrap();
    for stat in &stats {
        writeln!(
            out,
            "    StatInfo::new({}, {}, {}, {}, {}, {}, {}),",
            stat.csv_bits,
            stat.csv_param,
            stat.csv_signed,
            stat.save_bits,
            stat.save_param_bits,
            stat.save_add,
            stat.val_shift
        )
        .unwrap();
    }
    out.push_str("];\n\n#[rustfmt::skip]\npub(super) static ITEMS: [(u32, ItemInfo); ");
    writeln!(out, "{}] = [", entries.len()).unwrap();
    for (code, bits) in &entries {
        let text = String::from_utf8(code.to_le_bytes().to_vec()).expect("ascii item code");
        writeln!(
            out,
            "    (u32::from_le_bytes(*b\"{text}\"), ItemInfo::from_bits(0x{bits:04X})),"
        )
        .unwrap();
    }
    out.push_str("];\n");
    out
}

#[test]
fn the_version_105_tables_match_the_reference_txt_files() {
    let path = repo_root().join(GENERATED);
    let rendered = render();

    if std::env::var_os("UPDATE_TABLES").is_some() {
        std::fs::create_dir_all(path.parent().expect("parent directory")).expect("create dir");
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
