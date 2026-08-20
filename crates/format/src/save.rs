use crate::bits::{BitReader, BitWriter};
use crate::character::Character;
use crate::checksum;
use crate::error::{FormatError, SUPPORTED_VERSION};
use crate::item::Item;
use crate::sections::{IntroSection, QuestSection, SkillsSection, WaypointSection};
use crate::stats::{DemonStats, PlayerStats};

/// FORMAT.md §3.1
pub const FILE_MAGIC: u32 = 0xAA55_AA55;
/// FORMAT.md §3.9
pub const ITEMS_MAGIC: u16 = 0x4D4A;
/// FORMAT.md §3.11
pub const MERC_MAGIC: u16 = 0x666A;
/// FORMAT.md §3.12
pub const GOLEM_MAGIC: u16 = 0x666B;
/// FORMAT.md §3.13
pub const DEMON_MAGIC: u16 = 0x666C;

/// FORMAT.md §3.9
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemList(pub Vec<Item>);

impl ItemList {
    /// FORMAT.md §3.9
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let at = reader.byte_position()?;
        let magic = reader.read_u16()?;
        if magic != ITEMS_MAGIC {
            return Err(FormatError::BadMagic {
                section: "items",
                expected: u32::from(ITEMS_MAGIC),
                found: u32::from(magic),
                at,
            });
        }
        let count = reader.read_u16()?;
        let mut items = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            items.push(Item::read(reader)?);
        }
        Ok(Self(items))
    }

    /// FORMAT.md §3.9
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u16(ITEMS_MAGIC)?;
        writer.write_u16(u16::try_from(self.0.len()).map_err(|_| {
            FormatError::BadSectionSize {
                section: "items",
                expected: usize::from(u16::MAX),
                found: self.0.len(),
            }
        })?)?;
        for item in &self.0 {
            item.write(writer)?;
        }
        Ok(())
    }
}

/// FORMAT.md §3.10
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corpse {
    pub unknown: u32,
    pub x: u32,
    pub y: u32,
    pub items: ItemList,
}

/// FORMAT.md §3.10
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CorpseSection(pub Vec<Corpse>);

impl CorpseSection {
    /// FORMAT.md §3.10
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let at = reader.byte_position()?;
        let magic = reader.read_u16()?;
        if magic != ITEMS_MAGIC {
            return Err(FormatError::BadMagic {
                section: "corpses",
                expected: u32::from(ITEMS_MAGIC),
                found: u32::from(magic),
                at,
            });
        }
        let count = reader.read_u16()?;
        let mut corpses = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            corpses.push(Corpse {
                unknown: reader.read_u32()?,
                x: reader.read_u32()?,
                y: reader.read_u32()?,
                items: ItemList::read(reader)?,
            });
        }
        Ok(Self(corpses))
    }

    /// FORMAT.md §3.10
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u16(ITEMS_MAGIC)?;
        writer.write_u16(u16::try_from(self.0.len()).map_err(|_| {
            FormatError::BadSectionSize {
                section: "corpses",
                expected: usize::from(u16::MAX),
                found: self.0.len(),
            }
        })?)?;
        for corpse in &self.0 {
            writer.write_u32(corpse.unknown)?;
            writer.write_u32(corpse.x)?;
            writer.write_u32(corpse.y)?;
            corpse.items.write(writer)?;
        }
        Ok(())
    }
}

/// FORMAT.md §3.13
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemonData {
    pub kind: u16,
    pub class_id: u16,
    pub name_seed: u16,
    pub mon_flags: u16,
    pub stat_count: u16,
    pub unknown_0c: u8,
    pub unknown_0d: u8,
    pub unknown_0e: u16,
    pub unknown_10: [u8; 28],
    pub unknown_2c: [u32; 5],
    pub components: [u8; 16],
    pub mon_umod: [u8; 10],
    pub unknown_5a: [u8; 2],
    pub stats: DemonStats,
}

/// FORMAT.md §3.13
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemonSection {
    pub version: u16,
    pub padding: u8,
    pub demon: Option<DemonData>,
}

impl DemonSection {
    /// FORMAT.md §3.13
    fn read(reader: &mut BitReader<'_>) -> Result<Option<Self>, FormatError> {
        if reader.bits_left() < 48 {
            return Ok(None);
        }
        let start = reader.bit_position();
        let version = reader.read_u16()?;
        let magic = reader.read_u16()?;
        if magic != DEMON_MAGIC || version != 1 {
            reader.seek(start);
            return Ok(None);
        }
        let present = reader.read_u8()? != 0;
        let padding = reader.read_u8()?;
        let demon = if present {
            Some(Self::read_demon(reader)?)
        } else {
            None
        };
        Ok(Some(Self {
            version,
            padding,
            demon,
        }))
    }

    /// FORMAT.md §3.13
    fn read_demon(reader: &mut BitReader<'_>) -> Result<DemonData, FormatError> {
        let stat_bytes = usize::from(reader.read_u16()?);
        Ok(DemonData {
            kind: reader.read_u16()?,
            class_id: reader.read_u16()?.wrapping_sub(1),
            name_seed: reader.read_u16()?,
            mon_flags: reader.read_u16()?,
            stat_count: reader.read_u16()?,
            unknown_0c: reader.read_u8()?,
            unknown_0d: reader.read_u8()?,
            unknown_0e: reader.read_u16()?,
            unknown_10: reader.read_array::<28>()?,
            unknown_2c: reader.read_u32_array::<5>()?,
            components: reader.read_array::<16>()?,
            mon_umod: reader.read_array::<10>()?,
            unknown_5a: reader.read_array::<2>()?,
            stats: read_demon_stats(reader, stat_bytes)?,
        })
    }

    /// FORMAT.md §3.13
    fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u16(self.version)?;
        writer.write_u16(DEMON_MAGIC)?;
        writer.write_u8(u8::from(self.demon.is_some()))?;
        writer.write_u8(self.padding)?;

        let Some(demon) = &self.demon else {
            return Ok(());
        };

        let blob = demon.stats.write_blob()?;

        writer.write_u16(u16::try_from(blob.len()).map_err(|_| {
            FormatError::BadSectionSize {
                section: "demon stats",
                expected: usize::from(u16::MAX),
                found: blob.len(),
            }
        })?)?;
        writer.write_u16(demon.kind)?;
        writer.write_u16(demon.class_id.wrapping_add(1))?;
        writer.write_u16(demon.name_seed)?;
        writer.write_u16(demon.mon_flags)?;
        writer.write_u16(demon.stat_count)?;
        writer.write_u8(demon.unknown_0c)?;
        writer.write_u8(demon.unknown_0d)?;
        writer.write_u16(demon.unknown_0e)?;
        writer.write_bytes(&demon.unknown_10)?;
        for value in demon.unknown_2c {
            writer.write_u32(value)?;
        }
        writer.write_bytes(&demon.components)?;
        writer.write_bytes(&demon.mon_umod)?;
        writer.write_bytes(&demon.unknown_5a)?;
        writer.write_bytes(&blob)?;
        Ok(())
    }
}

/// FORMAT.md §3.11
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MercItems(pub Option<ItemList>);

/// FORMAT.md §3.12
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IronGolem(pub Option<Item>);

/// FORMAT.md §3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSave {
    pub declared_bytes: u32,
    pub declared_checksum: u32,
    pub character: Character,
    pub quests: QuestSection,
    pub waypoints: WaypointSection,
    pub intro: IntroSection,
    pub stats: PlayerStats,
    pub skills: SkillsSection,
    pub items: ItemList,
    pub corpses: CorpseSection,
    pub merc_items: Option<MercItems>,
    pub iron_golem: Option<IronGolem>,
    pub demon: Option<DemonSection>,
    pub trailing: Vec<u8>,
}

impl CharacterSave {
    /// FORMAT.md §3.1
    pub fn read(data: &[u8]) -> Result<Self, FormatError> {
        let mut reader = BitReader::new(data);
        let magic = reader.read_u32()?;
        if magic != FILE_MAGIC {
            return Err(FormatError::BadMagic {
                section: "character save",
                expected: FILE_MAGIC,
                found: magic,
                at: 0,
            });
        }
        let version = reader.read_u32()?;
        if version != SUPPORTED_VERSION {
            return Err(FormatError::UnsupportedVersion { version });
        }
        let declared_bytes = reader.read_u32()?;
        let declared_checksum = reader.read_u32()?;

        let character = Character::read(&mut reader)?;
        let quests = QuestSection::read(&mut reader)?;
        let waypoints = WaypointSection::read(&mut reader)?;
        let intro = IntroSection::read(&mut reader)?;

        let stats = PlayerStats::read_section(&mut reader)?;

        let skills = SkillsSection::read(&mut reader, usize::from(character.skill_count))?;
        let items = ItemList::read(&mut reader)?;
        let corpses = CorpseSection::read(&mut reader)?;

        let expansion = character.preview.game_version.is_expansion();
        let mut merc_items = None;
        let mut iron_golem = None;
        let mut demon = None;
        if expansion {
            merc_items = Some(read_merc_items(&mut reader, character.merc.is_hired())?);
            iron_golem = Some(read_iron_golem(&mut reader)?);
            if character.preview.game_version == crate::character::GameVersion::ReignOfTheWarlock {
                demon = DemonSection::read(&mut reader)?;
            }
        }

        let left = reader.bits_left() / 8;
        let trailing = reader.read_bytes(left)?.to_vec();

        Ok(Self {
            declared_bytes,
            declared_checksum,
            character,
            quests,
            waypoints,
            intro,
            stats,
            skills,
            items,
            corpses,
            merc_items,
            iron_golem,
            demon,
            trailing,
        })
    }

    /// FORMAT.md §3.1
    pub fn write(&self) -> Result<Vec<u8>, FormatError> {
        let mut writer = BitWriter::new();
        writer.write_u32(FILE_MAGIC)?;
        writer.write_u32(SUPPORTED_VERSION)?;
        writer.write_u32(0)?;
        writer.write_u32(0)?;

        self.character.write(&mut writer)?;
        self.quests.write(&mut writer)?;
        self.waypoints.write(&mut writer)?;
        self.intro.write(&mut writer)?;

        self.stats.write_section(&mut writer)?;

        self.skills.write(&mut writer)?;
        self.items.write(&mut writer)?;
        self.corpses.write(&mut writer)?;

        if let Some(merc) = &self.merc_items {
            writer.write_u16(MERC_MAGIC)?;
            if let Some(items) = &merc.0 {
                items.write(&mut writer)?;
            }
        }
        if let Some(golem) = &self.iron_golem {
            writer.write_u16(GOLEM_MAGIC)?;
            writer.write_u8(u8::from(golem.0.is_some()))?;
            if let Some(item) = &golem.0 {
                item.write(&mut writer)?;
            }
        }
        if let Some(demon) = &self.demon {
            demon.write(&mut writer)?;
        }
        if !self.trailing.is_empty() {
            writer.write_bytes(&self.trailing)?;
        }

        let written = writer.byte_len();
        let size = u32::try_from(written).map_err(|_| FormatError::FileSizeMismatch {
            declared: u32::MAX,
            actual: written,
        })?;
        writer.patch_u32(8, size);
        writer.patch_u32(12, 0);
        let mut data = writer.finish();
        let sum = checksum::compute(&data);
        data[12..16].copy_from_slice(&sum.to_le_bytes());
        Ok(data)
    }
}

/// FORMAT.md §3.13
fn read_demon_stats(
    reader: &mut BitReader<'_>,
    stat_bytes: usize,
) -> Result<DemonStats, FormatError> {
    if stat_bytes == 0 {
        return Ok(DemonStats::default());
    }
    DemonStats::read_blob(reader.read_bytes(stat_bytes)?)
}

/// FORMAT.md §3.11
fn read_merc_items(reader: &mut BitReader<'_>, hired: bool) -> Result<MercItems, FormatError> {
    let at = reader.byte_position()?;
    let magic = reader.read_u16()?;
    if magic != MERC_MAGIC {
        return Err(FormatError::BadMagic {
            section: "merc items",
            expected: u32::from(MERC_MAGIC),
            found: u32::from(magic),
            at,
        });
    }
    if hired {
        Ok(MercItems(Some(ItemList::read(reader)?)))
    } else {
        Ok(MercItems(None))
    }
}

/// FORMAT.md §3.12
fn read_iron_golem(reader: &mut BitReader<'_>) -> Result<IronGolem, FormatError> {
    let at = reader.byte_position()?;
    let magic = reader.read_u16()?;
    if magic != GOLEM_MAGIC {
        return Err(FormatError::BadMagic {
            section: "iron golem",
            expected: u32::from(GOLEM_MAGIC),
            found: u32::from(magic),
            at,
        });
    }
    if reader.read_u8()? == 0 {
        return Ok(IronGolem(None));
    }
    Ok(IronGolem(Some(Item::read(reader)?)))
}
