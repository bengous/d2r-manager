use std::fmt::Write as _;

use format::bits::Bits;
use format::character::{
    Character, CharacterClass, GameVersion, MercData, PreviewData, PreviewItem, Skill, SkillSide,
};
use format::code::ItemCode;
use format::item::{
    AffixSlot, BodyLocation, CharacterIdentity, ChronicleEntries, CompactContent, Durability,
    GroundMode, Item, ItemBody, ItemChronicle, ItemName, ItemPosition, ItemQuality, MagicAffix,
    NormalExtra, RareNames, SlotMode, StorePage, TypeSpecific,
};
use format::save::{CharacterSave, Corpse, DemonData, DemonSection, ItemList};
use format::sections::{IntroSection, QuestSection, SkillsSection, WaypointSection};
use format::stash::{ChronicleSection, StashSave, StashTab, TabBody, TabKind};
use format::stats::{ItemStat, ItemStats, PlayerStats, StatId};
use serde_json::{Map, Value, json};

const CLASS_SKILL_OFFSETS: [i64; 8] = [6, 36, 66, 96, 126, 221, 251, 373];

const ACT_ONE: [&str; 8] = [
    "Introduction",
    "DenOfEvil",
    "SistersBurialGrounds",
    "ToolsOfTheTrade",
    "TheSearchForCain",
    "TheForgottenTower",
    "SistersToTheSlaughter",
    "Completion",
];
const ACT_TWO: [&str; 8] = [
    "Introduction",
    "RadamentsLair",
    "TheHoradricStaff",
    "TaintedSun",
    "ArcaneSanctuary",
    "TheSummoner",
    "TheSevenTombs",
    "Completion",
];
const ACT_THREE: [&str; 8] = [
    "Introduction",
    "LamEsensTome",
    "KhalimsWill",
    "BladeOfTheOldReligion",
    "TheGoldenBird",
    "TheBlackenedTemple",
    "TheGuardian",
    "Completion",
];
const ACT_FOUR: [&str; 8] = [
    "Introduction",
    "TheFallenAngel",
    "TerrorsEnd",
    "Hellforge",
    "Completion",
    "Reserved1",
    "Reserved2",
    "Reserved3",
];
const ACT_FIVE: [&str; 16] = [
    "Introduction",
    "Reserved1",
    "Reserved2",
    "SiegeOnHarrogath",
    "RescueOnMountArreat",
    "PrisonOfIce",
    "BetrayalOfHarrogath",
    "RiteOfPassage",
    "EveOfDestruction",
    "Completion",
    "Reserved3",
    "Reserved4",
    "Reserved5",
    "Reserved6",
    "Reserved7",
    "Reserved8",
];

const NPC_NAMES: [&str; 34] = [
    "Gheed",
    "Akara",
    "Kashya",
    "WarrivActI",
    "Charsi",
    "CainActI",
    "WarrivActII",
    "Atma",
    "Drognan",
    "Fara",
    "Lysander",
    "Geglash",
    "MeshifActII",
    "Jerhyn",
    "Greiz",
    "Elzix",
    "CainActII",
    "CainActIII",
    "CainActIV",
    "Tyrael",
    "Asheara",
    "Hratli",
    "Alkor",
    "Ormus",
    "Izual",
    "Halbu",
    "MeshifActIII",
    "Natalya",
    "Larzuk",
    "Anya",
    "Malah",
    "Nihlathak",
    "Qualkehk",
    "CainActV",
];

fn hex(bytes: &[u8]) -> Value {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(text, "{byte:02x}").expect("string write");
    }
    Value::String(text)
}

fn named(name: &str, value: i64) -> Value {
    json!({ "name": name, "value": value })
}

fn unnamed(value: i64) -> Value {
    json!({ "value": value })
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

fn optional<T>(value: Option<T>, project: impl FnOnce(T) -> Value) -> Value {
    value.map_or(Value::Null, project)
}

fn bits_value<const N: u32>(value: Bits<N>) -> Value {
    Value::from(i64::from(value.get()))
}

fn skill(skill: Skill) -> Value {
    object(vec![
        ("ItemIndex", Value::from(i64::from(skill.item_index))),
        (
            "Side",
            match skill.side {
                SkillSide::Right => named("Right", 0),
                SkillSide::Left => named("Left", 1),
            },
        ),
        ("SkillId", bits_value(skill.id)),
    ])
}

fn class_name(class: CharacterClass) -> &'static str {
    match class {
        CharacterClass::Amazon => "Amazon",
        CharacterClass::Sorceress => "Sorceress",
        CharacterClass::Necromancer => "Necromancer",
        CharacterClass::Paladin => "Paladin",
        CharacterClass::Barbarian => "Barbarian",
        CharacterClass::Druid => "Druid",
        CharacterClass::Assassin => "Assassin",
        CharacterClass::Warlock => "Warlock",
    }
}

fn game_version(version: GameVersion) -> Value {
    let name = match version {
        GameVersion::None => "None",
        GameVersion::Classic => "Classic",
        GameVersion::Expansion => "Expansion",
        GameVersion::ReignOfTheWarlock => "ReignOfTheWarlock",
    };
    named(name, i64::from(version.code()))
}

fn merc(merc: MercData) -> Value {
    object(vec![
        ("Experience", Value::from(i64::from(merc.experience))),
        ("Flags", Value::from(i64::from(merc.flags))),
        ("HasMerc", Value::Bool(merc.is_hired())),
        ("HirelingId", Value::from(i64::from(merc.hireling_id))),
        ("IsDead", Value::Bool(merc.is_dead())),
        ("NameIndex", Value::from(i64::from(merc.name_index))),
        ("Seed", Value::from(i64::from(merc.seed))),
    ])
}

fn code_text(code: u32) -> String {
    String::from_utf8_lossy(&code.to_le_bytes())
        .trim_end_matches(['\0', ' '])
        .to_owned()
}

fn preview_item(item: PreviewItem) -> Value {
    object(vec![
        ("Code", Value::from(i64::from(item.code))),
        ("CodeString", Value::String(code_text(item.code))),
        ("FileIndex", Value::from(i64::from(item.file_index))),
        ("Flags", unnamed(i64::from(item.flags))),
        ("Quality", unnamed(i64::from(item.quality))),
        ("Transform", Value::from(i64::from(item.transform))),
    ])
}

#[expect(clippy::cast_possible_wrap, reason = "u64 timestamps compared as text")]
fn preview(preview: &PreviewData) -> Value {
    object(vec![
        (
            "ClassicExperience",
            Value::from(i64::from(preview.experiences[1])),
        ),
        ("ClassicSaveTime", Value::from(preview.save_times[1])),
        (
            "ExpansionExperience",
            Value::from(i64::from(preview.experiences[0])),
        ),
        ("ExpansionSaveTime", Value::from(preview.save_times[0])),
        (
            "Experiences",
            Value::Array(
                preview
                    .experiences
                    .iter()
                    .map(|value| Value::from(i64::from(*value)))
                    .collect(),
            ),
        ),
        ("GameVersion", game_version(preview.game_version)),
        (
            "GuildEmblemColor",
            Value::from(i64::from(preview.guild_emblem_color)),
        ),
        ("Head", preview_item(preview.head)),
        ("LeftHand", preview_item(preview.left_hand)),
        ("Name", Value::String(preview.name.to_string())),
        ("PreviewPadding", Value::from(i64::from(preview.padding))),
        ("RightHand", preview_item(preview.right_hand)),
        (
            "SaveTimes",
            Value::Array(
                preview
                    .save_times
                    .iter()
                    .map(|value| Value::from(*value))
                    .collect(),
            ),
        ),
        ("Torso", preview_item(preview.torso)),
        ("Unk1", Value::from(preview.reserved as i64)),
    ])
}

fn character(character: &Character) -> Value {
    object(vec![
        ("AppearanceGraphics", hex(&character.appearance_graphics)),
        ("AppearanceTints", hex(&character.appearance_tints)),
        (
            "Class",
            named(
                class_name(character.class),
                i64::from(character.class.code()),
            ),
        ),
        ("CreateTime", Value::from(i64::from(character.create_time))),
        ("Flags", unnamed(i64::from(character.flags))),
        (
            "LastSaveTime",
            Value::from(i64::from(character.last_save_time)),
        ),
        ("LeftSkill", skill(character.left_skill)),
        ("Level", Value::from(i64::from(character.level))),
        ("MapSeed", Value::from(i64::from(character.map_seed))),
        ("MercData", merc(character.merc)),
        ("Name", Value::String(String::new())),
        ("NumSkills", Value::from(i64::from(character.skill_count))),
        ("NumStats", Value::from(i64::from(character.stat_count))),
        ("PlayTime", Value::from(i64::from(character.play_time))),
        ("Preview", preview(&character.preview)),
        ("RightSkill", skill(character.right_skill)),
        (
            "SkillHotkeys",
            Value::Array(character.hotkeys.iter().map(|slot| skill(*slot)).collect()),
        ),
        ("SwitchLeftSkill", skill(character.switch_left_skill)),
        ("SwitchRightSkill", skill(character.switch_right_skill)),
        ("TownDifficulty", hex(&character.town_difficulty)),
        (
            "WeaponSwitch",
            Value::from(i64::from(character.weapon_switch)),
        ),
    ])
}

fn quest_act(names: &[&str], values: &[u16]) -> Value {
    let mut entries = Vec::new();
    for (name, value) in names.iter().zip(values) {
        entries.push((*name, unnamed(i64::from(*value))));
    }
    object(entries)
}

fn quests(section: &QuestSection) -> Value {
    let difficulty = |index: usize| {
        let data = &section.difficulties[index];
        object(vec![
            ("ActI", quest_act(&ACT_ONE, &data.act_one)),
            ("ActII", quest_act(&ACT_TWO, &data.act_two)),
            ("ActIII", quest_act(&ACT_THREE, &data.act_three)),
            ("ActIV", quest_act(&ACT_FOUR, &data.act_four)),
            ("ActV", quest_act(&ACT_FIVE, &data.act_five)),
        ])
    };
    object(vec![
        ("Hell", difficulty(2)),
        ("Nightmare", difficulty(1)),
        ("Normal", difficulty(0)),
        (
            "SectionSize",
            Value::from(i64::from(section.declared_bytes)),
        ),
        ("Version", Value::from(i64::from(section.version))),
    ])
}

fn waypoints(section: &WaypointSection) -> Value {
    let difficulty = |index: usize| {
        let data = &section.difficulties[index];
        object(vec![
            ("ActI", unnamed(i64::from(data.act_one.get()))),
            ("ActII", unnamed(i64::from(data.act_two.get()))),
            ("ActIII", unnamed(i64::from(data.act_three.get()))),
            ("ActIV", unnamed(i64::from(data.act_four.get()))),
            ("ActV", unnamed(i64::from(data.act_five.get()))),
            ("Header", Value::from(i64::from(data.header))),
            ("Padding", Value::from(i64::from(data.padding.get()))),
            ("Reserved", hex(&data.reserved)),
        ])
    };
    object(vec![
        ("Hell", difficulty(2)),
        ("Nightmare", difficulty(1)),
        ("Normal", difficulty(0)),
        (
            "SectionSize",
            Value::from(i64::from(section.declared_bytes)),
        ),
        ("Version", Value::from(i64::from(section.version))),
    ])
}

fn npc_bit(flags: [u8; 8], index: usize) -> bool {
    flags[index / 8] & (1 << (index % 8)) != 0
}

fn intro(section: &IntroSection) -> Value {
    let difficulty = |index: usize| {
        let quest = &section.quest_intro[index];
        let npc = &section.npc_intro[index];
        let mut entries = vec![("NpcIntroFlags", hex(npc)), ("QuestIntroFlags", hex(quest))];
        for (offset, name) in NPC_NAMES.iter().enumerate() {
            entries.push((
                name,
                object(vec![
                    ("NpcIntroShown", Value::Bool(npc_bit(*npc, offset + 1))),
                    ("QuestIntroShown", Value::Bool(npc_bit(*quest, offset + 1))),
                ]),
            ));
        }
        object(entries)
    };
    object(vec![
        (
            "Header",
            Value::from(i64::from(format::sections::INTRO_MAGIC)),
        ),
        ("Hell", difficulty(2)),
        ("Nightmare", difficulty(1)),
        ("Normal", difficulty(0)),
        (
            "SectionSize",
            Value::from(i64::from(section.declared_bytes)),
        ),
    ])
}

fn skills(section: &SkillsSection, class: CharacterClass) -> Value {
    object(vec![
        (
            "CharacterClass",
            named(class_name(class), i64::from(class.code())),
        ),
        (
            "SkillIdOffset",
            Value::from(CLASS_SKILL_OFFSETS[usize::from(class.code())]),
        ),
        ("SkillLevels", hex(&section.levels)),
    ])
}

fn player_stats(stats: &PlayerStats) -> Value {
    Value::Array(
        stats
            .0
            .iter()
            .map(|stat| {
                object(vec![
                    ("Id", unnamed(i64::from(stat.id.get()))),
                    ("Layer", Value::from(i64::from(stat.layer))),
                    ("Value", Value::from(stat.value)),
                ])
            })
            .collect(),
    )
}

fn stat_entry(id: StatId, layer: i64, value: i64) -> Value {
    object(vec![
        ("Id", unnamed(i64::from(id.get()))),
        ("Layer", Value::from(layer)),
        ("Value", Value::from(value)),
    ])
}

fn item_stats(stats: &ItemStats) -> Value {
    let mut out = Vec::new();
    for stat in &stats.0 {
        push_stat(&mut out, stat);
    }
    Value::Array(out)
}

fn push_stat(out: &mut Vec<Value>, stat: &ItemStat) {
    out.push(stat_entry(stat.id, i64::from(stat.param), stat.value));
    for (follower, value) in stat.id.paired().iter().zip(&stat.paired) {
        out.push(stat_entry(*follower, 0, *value));
    }
    if stat.id == StatId::POISON_MIN_DAMAGE {
        out.push(stat_entry(StatId::POISON_COUNT, 0, 1));
    }
}

fn mode_name(position: ItemPosition) -> (&'static str, i64) {
    match position {
        ItemPosition::Ground { mode, .. } => match mode {
            GroundMode::OnGround => ("OnGround", 3),
            GroundMode::Dropping => ("Dropping", 5),
        },
        ItemPosition::Slot { mode, .. } => match mode {
            SlotMode::Stored => ("Stored", 0),
            SlotMode::Equipped => ("Equipped", 1),
            SlotMode::InBelt => ("InBelt", 2),
            SlotMode::OnCursor => ("OnCursor", 4),
            SlotMode::Socketed => ("Socketed", 6),
        },
    }
}

fn body_name(body: BodyLocation) -> &'static str {
    match body {
        BodyLocation::None => "None",
        BodyLocation::Head => "Head",
        BodyLocation::Neck => "Neck",
        BodyLocation::Torso => "Torso",
        BodyLocation::RightArm => "RightArm",
        BodyLocation::LeftArm => "LeftArm",
        BodyLocation::RightRing => "RightRing",
        BodyLocation::LeftRing => "LeftRing",
        BodyLocation::Belt => "Belt",
        BodyLocation::Feet => "Feet",
        BodyLocation::Gloves => "Gloves",
        BodyLocation::RightHand => "RightHand",
        BodyLocation::LeftHand => "LeftHand",
    }
}

fn store_page(page: StorePage) -> Value {
    match page {
        StorePage::None => named("Null", 255),
        StorePage::Inventory => named("Inventory", 0),
        StorePage::Equip => named("Equip", 1),
        StorePage::Trade => named("Trade", 2),
        StorePage::Cube => named("Cube", 3),
        StorePage::Stash => named("Stash", 4),
        StorePage::Belt => named("Belt", 5),
        StorePage::Seventh => unnamed(6),
    }
}

fn position(position: ItemPosition) -> Value {
    let (name, code) = mode_name(position);
    let (ground_x, ground_y, body, column, row, page) = match position {
        ItemPosition::Ground { x, y, .. } => (
            i64::from(x),
            i64::from(y),
            BodyLocation::None,
            0,
            0,
            StorePage::None,
        ),
        ItemPosition::Slot {
            body, cell, page, ..
        } => (
            0,
            0,
            body,
            i64::from(cell.column()),
            i64::from(cell.row()),
            page,
        ),
    };
    object(vec![
        (
            "BodyLocation",
            named(body_name(body), i64::from(body.code())),
        ),
        ("GroundX", Value::from(ground_x)),
        ("GroundY", Value::from(ground_y)),
        ("InvX", Value::from(column)),
        ("InvY", Value::from(row)),
        (
            "IsOnGround",
            Value::Bool(matches!(position, ItemPosition::Ground { .. })),
        ),
        ("Mode", named(name, code)),
        ("StorePage", store_page(page)),
    ])
}

fn quality_name(quality: &ItemQuality) -> &'static str {
    match quality {
        ItemQuality::Inferior(_) => "Inferior",
        ItemQuality::Normal(_) => "Normal",
        ItemQuality::Superior(_) => "Superior",
        ItemQuality::Magic { .. } => "Magic",
        ItemQuality::Set { .. } => "Set",
        ItemQuality::Rare(_) => "Rare",
        ItemQuality::Unique(_) => "Unique",
        ItemQuality::Craft(_) => "Craft",
        ItemQuality::Tempered { .. } => "Tempered",
    }
}

fn affix_list(affixes: &[AffixSlot; 3], prefix: bool) -> Value {
    Value::Array(
        affixes
            .iter()
            .map(|slot| {
                let value = if prefix { slot.prefix } else { slot.suffix };
                Value::from(i64::from(value.map_or(0, Bits::get)))
            })
            .collect(),
    )
}

fn rare_names(names: &RareNames) -> Value {
    object(vec![
        ("FileIndex", Value::Null),
        ("Prefixes", affix_list(&names.affixes, true)),
        ("RarePrefixId", bits_value(names.prefix)),
        ("RareSuffixId", bits_value(names.suffix)),
        ("Suffixes", affix_list(&names.affixes, false)),
    ])
}

fn quality_data(quality: &ItemQuality) -> Value {
    match quality {
        ItemQuality::Inferior(index) => object(vec![
            ("FileIndex", bits_value(*index)),
            ("InferiorFileIndex", bits_value(*index)),
        ]),
        ItemQuality::Superior(index) => object(vec![
            ("FileIndex", bits_value(*index)),
            ("SuperiorFileIndex", bits_value(*index)),
        ]),
        ItemQuality::Magic { prefix, suffix } => object(vec![
            ("FileIndex", Value::Null),
            ("PrefixId", bits_value(*prefix)),
            ("SuffixId", bits_value(*suffix)),
        ]),
        ItemQuality::Set { file_index, .. } | ItemQuality::Unique(file_index) => object(vec![
            ("FileIndex", bits_value(*file_index)),
            ("SetUniqueFileIndex", bits_value(*file_index)),
        ]),
        ItemQuality::Rare(names) | ItemQuality::Craft(names) => rare_names(names),
        ItemQuality::Tempered { prefix, suffix } => object(vec![
            ("FileIndex", Value::Null),
            ("RarePrefixId", bits_value(*prefix)),
            ("RareSuffixId", bits_value(*suffix)),
        ]),
        ItemQuality::Normal(extra) => match extra {
            NormalExtra::Absent => Value::Null,
            NormalExtra::Charm(affix) => {
                let (has_prefix, prefix, suffix) = match affix {
                    MagicAffix::Prefix(value) => (true, value.get(), 0),
                    MagicAffix::Suffix(value) => (false, 0, value.get()),
                };
                object(vec![
                    ("FileIndex", Value::Null),
                    ("HasPrefix", Value::Bool(has_prefix)),
                    ("PrefixId", Value::from(i64::from(prefix))),
                    ("SuffixId", Value::from(i64::from(suffix))),
                ])
            }
            NormalExtra::BodyPart(index) => object(vec![
                ("BodyPartFileIndex", bits_value(*index)),
                ("FileIndex", bits_value(*index)),
            ]),
            NormalExtra::ScrollOrBook(spell) => object(vec![
                ("FileIndex", Value::Null),
                ("SpellType", bits_value(*spell)),
            ]),
        },
    }
}

fn identity(identity: CharacterIdentity) -> Value {
    object(vec![
        ("CharacterId", Value::from(i64::from(identity.character_id))),
        (
            "GameAccountId",
            Value::from(i64::from(identity.game_account_id)),
        ),
    ])
}

fn chronicle_parts(chronicle: Option<&ItemChronicle>) -> (Value, Value, Value) {
    let Some(chronicle) = chronicle else {
        return (Value::Null, Value::Null, Value::Null);
    };
    let id = bits_value(chronicle.id);
    match &chronicle.entries {
        ChronicleEntries::Compact(recipient) => {
            (id, Value::Null, Value::Array(vec![identity(*recipient)]))
        }
        ChronicleEntries::Dated {
            timestamp,
            recipients,
        } => {
            let list = if recipients.is_empty() {
                Value::Null
            } else {
                Value::Array(recipients.iter().map(|entry| identity(*entry)).collect())
            };
            (id, Value::from(i64::from(*timestamp)), list)
        }
    }
}

struct ItemFields {
    advanced_stash_stack: Value,
    auto_affix: Value,
    defense: Value,
    durability: Value,
    max_durability: Value,
    ear_file_index: i64,
    ear_level: i64,
    ear_name: String,
    gold_amount: i64,
    player_gold: bool,
    code: ItemCode,
    level: i64,
    seed: i64,
    personalized: String,
    quality: Value,
    quality_data: Value,
    quantity: Value,
    quest_difficulty: Value,
    realm: Value,
    runeword_id: Value,
    runeword_stats: Value,
    set_bonus: Value,
    set_mask: Value,
    sockets: Value,
    stats: Value,
    variable_gfx: Value,
    chronicle_id: Value,
    chronicle_timestamp: Value,
    chronicle_recipients: Value,
}

fn realm_value(realm: Option<&[u32; 4]>) -> Value {
    optional(realm, |values| {
        Value::Array(
            values
                .iter()
                .map(|value| Value::from(i64::from(*value)))
                .collect(),
        )
    })
}

fn durability_parts(durability: Durability) -> (Value, Value) {
    match durability {
        Durability::Indestructible => (Value::from(0), Value::Null),
        Durability::Limited { max, current } => (Value::from(max), Value::from(current)),
    }
}

fn compact_fields(
    body: &format::item::CompactBody,
    chronicle: (Value, Value, Value),
) -> ItemFields {
    let (chronicle_id, chronicle_timestamp, chronicle_recipients) = chronicle;

    let (code, ear, gold) = match &body.content {
        CompactContent::Ear(ear) => (ItemCode::EAR, Some(ear), None),
        CompactContent::Code { code, gold } => (*code, None, gold.as_ref()),
    };
    ItemFields {
        advanced_stash_stack: optional(body.advanced_stash_stack, bits_value),
        auto_affix: Value::Null,
        defense: Value::Null,
        durability: Value::Null,
        max_durability: Value::Null,
        ear_file_index: ear.map_or(0, |ear| i64::from(ear.file_index.get())),
        ear_level: ear.map_or(0, |ear| i64::from(ear.level.get())),
        ear_name: ear.map(|ear| ear.name.to_string()).unwrap_or_default(),
        gold_amount: gold.map_or(0, |gold| i64::from(gold.amount.get())),
        player_gold: gold.is_some_and(|gold| gold.player_gold),
        code,
        level: 1,
        seed: 0,
        personalized: String::new(),
        quality: named("Normal", 2),
        quality_data: Value::Null,
        quantity: Value::Null,
        quest_difficulty: optional(body.quest_difficulty, Value::from),
        realm: realm_value(body.realm.as_ref()),
        runeword_id: Value::Null,
        runeword_stats: Value::Null,
        set_bonus: Value::Array(Vec::new()),
        set_mask: Value::Null,
        sockets: Value::Array(Vec::new()),
        stats: Value::Array(Vec::new()),
        variable_gfx: Value::Null,
        chronicle_id,
        chronicle_timestamp,
        chronicle_recipients,
    }
}

fn complete_fields(
    body: &format::item::CompleteBody,
    chronicle: (Value, Value, Value),
) -> ItemFields {
    let (chronicle_id, chronicle_timestamp, chronicle_recipients) = chronicle;

    let (defense, max_durability, durability) = match body.type_specific {
        TypeSpecific::Armor {
            defense,
            durability,
        } => {
            let (max, current) = durability_parts(durability);
            (Value::from(defense), max, current)
        }
        TypeSpecific::Weapon(durability) => {
            let (max, current) = durability_parts(durability);
            (Value::Null, max, current)
        }
        TypeSpecific::Absent | TypeSpecific::Gold(_) => (Value::Null, Value::Null, Value::Null),
    };
    let gold = match body.type_specific {
        TypeSpecific::Gold(gold) => Some(gold),
        _ => None,
    };
    let (ear, personalized) = match &body.name {
        ItemName::Ear(ear) => (Some(ear), String::new()),
        ItemName::Personalized(name) => (None, name.to_string()),
        ItemName::Absent => (None, String::new()),
    };
    let capacity = body.sockets.capacity.unwrap_or(0) as usize;
    let mut sockets: Vec<Value> = body.sockets.filled.iter().map(item_value).collect();
    while sockets.len() < capacity {
        sockets.push(Value::Null);
    }
    let set_mask = match &body.quality {
        ItemQuality::Set { bonuses, .. } => bits_value(bonuses.mask),
        _ => Value::Null,
    };
    let set_bonus = match &body.quality {
        ItemQuality::Set { bonuses, .. } => {
            Value::Array(bonuses.lists.iter().map(item_stats).collect())
        }
        _ => Value::Array(Vec::new()),
    };
    ItemFields {
        advanced_stash_stack: optional(body.advanced_stash_stack, bits_value),
        auto_affix: optional(body.auto_affix, bits_value),
        defense,
        durability,
        max_durability,
        ear_file_index: ear.map_or(0, |ear| i64::from(ear.file_index.get())),
        ear_level: ear.map_or(0, |ear| i64::from(ear.level.get())),
        ear_name: ear.map(|ear| ear.name.to_string()).unwrap_or_default(),
        gold_amount: gold.map_or(0, |gold| i64::from(gold.amount.get())),
        player_gold: gold.is_some_and(|gold| gold.player_gold),
        code: body.code,
        level: i64::from(body.level.get()),
        seed: i64::from(body.seed),
        personalized,
        quality: named(quality_name(&body.quality), i64::from(body.quality.code())),
        quality_data: quality_data(&body.quality),
        quantity: optional(body.quantity, bits_value),
        quest_difficulty: Value::Null,
        realm: realm_value(body.realm.as_ref()),
        runeword_id: optional(body.runeword.as_ref(), |runeword| bits_value(runeword.id)),
        runeword_stats: optional(body.runeword.as_ref(), |runeword| {
            item_stats(&runeword.stats)
        }),
        set_bonus,
        set_mask,
        sockets: Value::Array(sockets),
        stats: item_stats(&body.stats),
        variable_gfx: optional(body.variable_gfx, bits_value),
        chronicle_id,
        chronicle_timestamp,
        chronicle_recipients,
    }
}

fn item_fields(item: &Item) -> ItemFields {
    match &item.body {
        ItemBody::Compact(body) => compact_fields(body, (Value::Null, Value::Null, Value::Null)),
        ItemBody::Complete(body) => complete_fields(body, chronicle_parts(body.chronicle.as_ref())),
    }
}

pub(crate) fn item_value(item: &Item) -> Value {
    let fields = item_fields(item);
    object(vec![
        ("AdvancedStashStackSize", fields.advanced_stash_stack),
        ("AutoAffixId", fields.auto_affix),
        ("ChronicleId", fields.chronicle_id),
        ("ChronicleRecipients", fields.chronicle_recipients),
        ("ChronicleTimestamp", fields.chronicle_timestamp),
        ("Defense", fields.defense),
        ("Durability", fields.durability),
        ("EarFileIndex", Value::from(fields.ear_file_index)),
        ("EarLevel", Value::from(fields.ear_level)),
        ("EarName", Value::String(fields.ear_name)),
        ("Flags", unnamed(i64::from(item.flags.bits()))),
        ("GoldAmount", Value::from(fields.gold_amount)),
        ("ItemCode", Value::from(i64::from(fields.code.as_u32()))),
        ("ItemCodeString", Value::String(fields.code.to_string())),
        ("ItemLevel", Value::from(fields.level)),
        ("ItemSeed", Value::from(fields.seed)),
        ("MaxDurability", fields.max_durability),
        ("PersonalizedName", Value::String(fields.personalized)),
        ("PlayerGold", Value::Bool(fields.player_gold)),
        ("Position", position(item.position)),
        ("Quality", fields.quality),
        ("QualityData", fields.quality_data),
        ("Quantity", fields.quantity),
        ("QuestDifficulty", fields.quest_difficulty),
        ("RealmData", fields.realm),
        ("RunewordId", fields.runeword_id),
        ("RunewordStats", fields.runeword_stats),
        ("SetBonusStats", fields.set_bonus),
        ("SetItemMask", fields.set_mask),
        ("Sockets", fields.sockets),
        ("Stats", fields.stats),
        ("VariableGfxId", fields.variable_gfx),
        ("Version", Value::from(i64::from(item.version.number()))),
    ])
}

fn item_list(list: &ItemList) -> Value {
    Value::Array(list.0.iter().map(item_value).collect())
}

fn corpse(corpse: &Corpse) -> Value {
    object(vec![
        ("Items", item_list(&corpse.items)),
        ("Unknown", Value::from(i64::from(corpse.unknown))),
        ("X", Value::from(i64::from(corpse.x))),
        ("Y", Value::from(i64::from(corpse.y))),
    ])
}

fn demon_data(demon: &DemonData) -> Value {
    object(vec![
        ("ClassId", Value::from(i64::from(demon.class_id))),
        ("Components", hex(&demon.components)),
        ("DemonType", unnamed(i64::from(demon.kind))),
        ("MonFlags", Value::from(i64::from(demon.mon_flags))),
        ("MonUMod", hex(&demon.mon_umod)),
        ("NameSeed", Value::from(i64::from(demon.name_seed))),
        ("NumStats", Value::from(i64::from(demon.stat_count))),
        (
            "Stats",
            Value::Array(
                demon
                    .stats
                    .0
                    .iter()
                    .map(|stat| {
                        object(vec![
                            ("Id", unnamed(i64::from(stat.id.get()))),
                            ("Layer", Value::from(i64::from(stat.layer))),
                            ("Value", Value::from(stat.value)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("Unknown0C", Value::from(i64::from(demon.unknown_0c))),
        ("Unknown0D", Value::from(i64::from(demon.unknown_0d))),
        ("Unknown0E", Value::from(i64::from(demon.unknown_0e))),
        ("Unknown10", hex(&demon.unknown_10)),
        ("Unknown2C", Value::from(i64::from(demon.unknown_2c[0]))),
        ("Unknown30", Value::from(i64::from(demon.unknown_2c[1]))),
        ("Unknown34", Value::from(i64::from(demon.unknown_2c[2]))),
        ("Unknown38", Value::from(i64::from(demon.unknown_2c[3]))),
        ("Unknown3C", Value::from(i64::from(demon.unknown_2c[4]))),
        ("Unknown5A", hex(&demon.unknown_5a)),
    ])
}

fn demon(section: &DemonSection) -> Value {
    object(vec![
        ("DemonData", optional(section.demon.as_ref(), demon_data)),
        ("HasDemon", Value::Bool(section.demon.is_some())),
        ("Padding", Value::from(i64::from(section.padding))),
        ("SectionVersion", Value::from(i64::from(section.version))),
    ])
}

pub(crate) fn character_save(save: &CharacterSave) -> Value {
    object(vec![
        ("Character", character(&save.character)),
        ("Checksum", Value::from(i64::from(save.declared_checksum))),
        (
            "Corpses",
            Value::Array(save.corpses.0.iter().map(corpse).collect()),
        ),
        ("Demon", optional(save.demon.as_ref(), demon)),
        ("FileSize", Value::from(i64::from(save.declared_bytes))),
        (
            "IronGolem",
            optional(save.iron_golem.as_ref(), |golem| {
                object(vec![("GolemItem", optional(golem.0.as_ref(), item_value))])
            }),
        ),
        ("Items", item_list(&save.items)),
        (
            "MercItems",
            optional(save.merc_items.as_ref(), |merc| {
                object(vec![(
                    "Items",
                    merc.0
                        .as_ref()
                        .map_or_else(|| Value::Array(Vec::new()), item_list),
                )])
            }),
        ),
        ("PlayerIntro", intro(&save.intro)),
        ("Quests", quests(&save.quests)),
        ("Skills", skills(&save.skills, save.character.class)),
        ("Stats", player_stats(&save.stats)),
        ("TrailingData", hex(&save.trailing)),
        ("Version", Value::from(105)),
        ("Waypoints", waypoints(&save.waypoints)),
    ])
}

fn chronicle_entries(entries: &[format::stash::ChronicleEntry]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|entry| {
                object(vec![
                    ("ItemId", Value::from(i64::from(entry.item_id))),
                    ("Source", Value::from(i64::from(entry.source))),
                    ("Timestamp", Value::from(i64::from(entry.timestamp))),
                ])
            })
            .collect(),
    )
}

fn chronicle(section: &ChronicleSection, trailing: &[u8]) -> Value {
    object(vec![
        ("Reserved", hex(&section.reserved)),
        ("RunewordEntries", chronicle_entries(&section.runewords)),
        ("SetEntries", chronicle_entries(&section.sets)),
        ("TrailingData", hex(trailing)),
        ("UniqueEntries", chronicle_entries(&section.uniques)),
        ("Version", Value::from(i64::from(section.version))),
    ])
}

fn tab_kind(kind: TabKind) -> Value {
    let name = match kind {
        TabKind::Normal => "Normal",
        TabKind::AdvancedStash => "AdvancedStash",
        TabKind::Chronicle => "Chronicle",
    };
    named(name, i64::from(kind.code()))
}

fn stash_tab(tab: &StashTab) -> Value {
    let (chronicle_value, items) = match &tab.body {
        TabBody::Items(items) => (Value::Null, item_list(items)),
        TabBody::Chronicle(section) => {
            (chronicle(section, &tab.trailing), Value::Array(Vec::new()))
        }
    };
    object(vec![
        ("Chronicle", chronicle_value),
        ("Gold", Value::from(i64::from(tab.gold))),
        ("ItemFormat", Value::from(i64::from(tab.item_format))),
        ("Items", items),
        ("Reserved", hex(&tab.reserved)),
        ("Season", Value::from(i64::from(tab.season))),
        ("StashFormat", Value::from(i64::from(tab.stash_format))),
        ("TabType", tab_kind(tab.kind)),
    ])
}

pub(crate) fn stash_save(save: &StashSave) -> Value {
    Value::Array(save.tabs.iter().map(stash_tab).collect())
}
