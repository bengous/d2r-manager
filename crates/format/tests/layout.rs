mod corpus;

use corpus::fixture_bytes;
use format::bits::BitReader;
use format::character::{self, Character};
use format::sections::{self, IntroSection, QuestSection, SkillsSection, WaypointSection};
use format::stats::PlayerStats;

const LINUX: &str = "v105/linux";
const FIXTURE: &str = "Zealer.d2s";
const HEADER_BYTES: usize = 16;

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn u64_at(data: &[u8], offset: usize) -> u64 {
    u64::from(u32_at(data, offset)) | u64::from(u32_at(data, offset + 4)) << 32
}

#[test]
fn the_character_section_fields_sit_at_the_offsets_of_format_md_3_2() {
    let data = fixture_bytes(LINUX, FIXTURE);
    let mut reader = BitReader::new(&data);
    reader.seek(HEADER_BYTES * 8);
    let character = Character::read(&mut reader).expect("character section");

    let base = HEADER_BYTES;
    assert_eq!(character.weapon_switch, u32_at(&data, base), "offset 0");
    assert_eq!(character.flags, u32_at(&data, base + 4), "offset 4");
    assert_eq!(character.class.code(), data[base + 8], "offset 8");
    assert_eq!(character.stat_count, data[base + 9], "offset 9");
    assert_eq!(character.skill_count, data[base + 10], "offset 10");
    assert_eq!(character.level, data[base + 11], "offset 11");
    assert_eq!(character.create_time, u32_at(&data, base + 12), "offset 12");
    assert_eq!(
        character.last_save_time,
        u32_at(&data, base + 16),
        "offset 16"
    );
    assert_eq!(character.play_time, u32_at(&data, base + 20), "offset 20");

    let hotkey = u32_at(&data, base + 24);
    assert_eq!(
        character.hotkeys[0].id.get(),
        hotkey & 0x7FFF,
        "offset 24, 15 bits of skill id"
    );
    assert_eq!(
        u32::from(character.hotkeys[0].item_index),
        hotkey >> 16,
        "offset 26"
    );
    assert_eq!(
        character.left_skill.id.get(),
        u32_at(&data, base + 88) & 0x7FFF,
        "offset 88"
    );
    assert_eq!(
        character.switch_right_skill.id.get(),
        u32_at(&data, base + 100) & 0x7FFF,
        "offset 100"
    );

    assert_eq!(
        character.appearance_graphics,
        data[base + 104..base + 120],
        "offset 104"
    );
    assert_eq!(
        character.appearance_tints,
        data[base + 120..base + 136],
        "offset 120"
    );
    assert_eq!(
        character.town_difficulty,
        data[base + 136..base + 139],
        "offset 136"
    );
    assert_eq!(character.map_seed, u32_at(&data, base + 139), "offset 139");

    assert_eq!(
        character.merc.flags,
        u32_at(&data, base + 143),
        "offset 143"
    );
    assert_eq!(character.merc.seed, u32_at(&data, base + 147), "offset 147");
    assert_eq!(
        character.merc.name_index,
        u16_at(&data, base + 151),
        "offset 151"
    );
    assert_eq!(
        character.merc.hireling_id,
        u16_at(&data, base + 153),
        "offset 153"
    );
    assert_eq!(
        character.merc.experience,
        u32_at(&data, base + 155),
        "offset 155"
    );

    assert_eq!(
        reader.byte_position().expect("aligned"),
        base + character::SECTION_BYTES,
        "the character section spans 387 bytes"
    );
}

#[test]
fn the_preview_fields_sit_at_the_offsets_of_format_md_3_3() {
    let data = fixture_bytes(LINUX, FIXTURE);
    let mut reader = BitReader::new(&data);
    reader.seek(HEADER_BYTES * 8);
    let character = Character::read(&mut reader).expect("character section");
    let preview = &character.preview;

    let base = HEADER_BYTES + 159;
    assert_eq!(preview.save_times[0], u64_at(&data, base), "offset 0");
    assert_eq!(preview.save_times[5], u64_at(&data, base + 40), "offset 40");
    assert_eq!(
        preview.experiences[0],
        u32_at(&data, base + 48),
        "offset 48"
    );
    assert_eq!(
        preview.experiences[5],
        u32_at(&data, base + 68),
        "offset 68"
    );
    assert_eq!(preview.guild_emblem_color, data[base + 72], "offset 72");
    assert_eq!(preview.game_version.code(), data[base + 73], "offset 73");
    assert_eq!(preview.padding, u16_at(&data, base + 74), "offset 74");
    assert_eq!(
        preview.left_hand.code,
        u32_at(&data, base + 76),
        "offset 76"
    );
    assert_eq!(
        preview.right_hand.code,
        u32_at(&data, base + 88),
        "offset 88"
    );
    assert_eq!(preview.torso.code, u32_at(&data, base + 100), "offset 100");
    assert_eq!(preview.head.code, u32_at(&data, base + 112), "offset 112");
    assert_eq!(
        preview.name.bytes(),
        &data[base + 124..base + 220],
        "offset 124, 96 bytes"
    );
    assert_eq!(preview.reserved, u64_at(&data, base + 220), "offset 220");
}

#[test]
fn the_fixed_sections_span_the_sizes_of_format_md_3_4_to_3_6() {
    let data = fixture_bytes(LINUX, FIXTURE);
    let mut reader = BitReader::new(&data);
    reader.seek(HEADER_BYTES * 8);

    let character = Character::read(&mut reader).expect("character");
    let after_character = HEADER_BYTES + character::SECTION_BYTES;
    assert_eq!(reader.byte_position().expect("aligned"), after_character);
    assert_eq!(
        after_character, 403,
        "the version 105 header spans 403 bytes"
    );

    let quests = QuestSection::read(&mut reader).expect("quests");
    assert_eq!(usize::from(quests.declared_bytes), sections::QUEST_BYTES);
    assert_eq!(
        reader.byte_position().expect("aligned"),
        after_character + sections::QUEST_BYTES
    );

    let waypoints = WaypointSection::read(&mut reader).expect("waypoints");
    assert_eq!(
        usize::from(waypoints.declared_bytes),
        sections::WAYPOINT_BYTES
    );
    assert_eq!(
        reader.byte_position().expect("aligned"),
        after_character + sections::QUEST_BYTES + sections::WAYPOINT_BYTES
    );

    let intro = IntroSection::read(&mut reader).expect("intro");
    assert_eq!(usize::from(intro.declared_bytes), sections::INTRO_BYTES);
    let after_intro =
        after_character + sections::QUEST_BYTES + sections::WAYPOINT_BYTES + sections::INTRO_BYTES;
    assert_eq!(reader.byte_position().expect("aligned"), after_intro);

    let stats = PlayerStats::read_section(&mut reader).expect("stats");
    assert!(!stats.0.is_empty(), "a character carries stats");

    let skills = SkillsSection::read(&mut reader, usize::from(character.skill_count))
        .expect("skills section");
    assert_eq!(skills.levels.len(), 30, "thirty skills per class");
}
