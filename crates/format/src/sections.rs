use crate::bits::{BitReader, BitWriter, Bits};
use crate::error::FormatError;

/// FORMAT.md §3.4
pub const QUEST_MAGIC: u32 = 0x216F_6F57;
/// FORMAT.md §3.4
pub const QUEST_BYTES: usize = 298;
/// FORMAT.md §3.5
pub const WAYPOINT_MAGIC: u16 = 0x5357;
/// FORMAT.md §3.5
pub const WAYPOINT_BYTES: usize = 80;
/// FORMAT.md §3.6
pub const INTRO_MAGIC: u16 = 0x7701;
/// FORMAT.md §3.6
pub const INTRO_BYTES: usize = 52;
/// FORMAT.md §3.8
pub const SKILLS_MAGIC: u16 = 0x6669;
/// FORMAT.md §3.7
pub const STATS_MAGIC: u16 = 0x6667;

fn expect_magic_u16(
    reader: &mut BitReader<'_>,
    section: &'static str,
    expected: u16,
) -> Result<(), FormatError> {
    let at = reader.byte_position()?;
    let found = reader.read_u16()?;
    if found == expected {
        return Ok(());
    }
    Err(FormatError::BadMagic {
        section,
        expected: u32::from(expected),
        found: u32::from(found),
        at,
    })
}

fn expect_magic_u32(
    reader: &mut BitReader<'_>,
    section: &'static str,
    expected: u32,
) -> Result<(), FormatError> {
    let at = reader.byte_position()?;
    let found = reader.read_u32()?;
    if found == expected {
        return Ok(());
    }
    Err(FormatError::BadMagic {
        section,
        expected,
        found,
        at,
    })
}

/// FORMAT.md §3.4
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDifficulty {
    pub act_one: [u16; 8],
    pub act_two: [u16; 8],
    pub act_three: [u16; 8],
    pub act_four: [u16; 8],
    pub act_five: [u16; 16],
}

fn read_flags<const N: usize>(reader: &mut BitReader<'_>) -> Result<[u16; N], FormatError> {
    let mut out = [0u16; N];
    for slot in &mut out {
        *slot = reader.read_u16()?;
    }
    Ok(out)
}

fn write_flags(writer: &mut BitWriter, flags: &[u16]) -> Result<(), FormatError> {
    for value in flags {
        writer.write_u16(*value)?;
    }
    Ok(())
}

impl QuestDifficulty {
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        Ok(Self {
            act_one: read_flags::<8>(reader)?,
            act_two: read_flags::<8>(reader)?,
            act_three: read_flags::<8>(reader)?,
            act_four: read_flags::<8>(reader)?,
            act_five: read_flags::<16>(reader)?,
        })
    }

    fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        write_flags(writer, &self.act_one)?;
        write_flags(writer, &self.act_two)?;
        write_flags(writer, &self.act_three)?;
        write_flags(writer, &self.act_four)?;
        write_flags(writer, &self.act_five)
    }
}

/// FORMAT.md §3.4
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSection {
    pub version: u32,
    pub declared_bytes: u16,
    pub difficulties: [QuestDifficulty; 3],
}

impl QuestSection {
    /// FORMAT.md §3.4
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        expect_magic_u32(reader, "quests", QUEST_MAGIC)?;
        Ok(Self {
            version: reader.read_u32()?,
            declared_bytes: reader.read_u16()?,
            difficulties: [
                QuestDifficulty::read(reader)?,
                QuestDifficulty::read(reader)?,
                QuestDifficulty::read(reader)?,
            ],
        })
    }

    /// FORMAT.md §3.4
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u32(QUEST_MAGIC)?;
        writer.write_u32(self.version)?;
        writer.write_u16(self.declared_bytes)?;
        for difficulty in &self.difficulties {
            difficulty.write(writer)?;
        }
        Ok(())
    }
}

/// FORMAT.md §3.5
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaypointDifficulty {
    pub header: u16,
    pub act_one: Bits<9>,
    pub act_two: Bits<9>,
    pub act_three: Bits<9>,
    pub act_four: Bits<3>,
    pub act_five: Bits<9>,
    pub padding: Bits<9>,
    pub reserved: [u8; 16],
}

impl WaypointDifficulty {
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        Ok(Self {
            header: reader.read_u16()?,
            act_one: reader.read::<9>()?,
            act_two: reader.read::<9>()?,
            act_three: reader.read::<9>()?,
            act_four: reader.read::<3>()?,
            act_five: reader.read::<9>()?,
            padding: reader.read::<9>()?,
            reserved: reader.read_array::<16>()?,
        })
    }

    fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u16(self.header)?;
        writer.write(self.act_one)?;
        writer.write(self.act_two)?;
        writer.write(self.act_three)?;
        writer.write(self.act_four)?;
        writer.write(self.act_five)?;
        writer.write(self.padding)?;
        writer.write_bytes(&self.reserved)?;
        Ok(())
    }
}

/// FORMAT.md §3.5
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaypointSection {
    pub version: u32,
    pub declared_bytes: u16,
    pub difficulties: [WaypointDifficulty; 3],
}

impl WaypointSection {
    /// FORMAT.md §3.5
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        expect_magic_u16(reader, "waypoints", WAYPOINT_MAGIC)?;
        Ok(Self {
            version: reader.read_u32()?,
            declared_bytes: reader.read_u16()?,
            difficulties: [
                WaypointDifficulty::read(reader)?,
                WaypointDifficulty::read(reader)?,
                WaypointDifficulty::read(reader)?,
            ],
        })
    }

    /// FORMAT.md §3.5
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u16(WAYPOINT_MAGIC)?;
        writer.write_u32(self.version)?;
        writer.write_u16(self.declared_bytes)?;
        for difficulty in &self.difficulties {
            difficulty.write(writer)?;
        }
        Ok(())
    }
}

/// FORMAT.md §3.6
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntroSection {
    pub declared_bytes: u16,
    pub quest_intro: [[u8; 8]; 3],
    pub npc_intro: [[u8; 8]; 3],
}

impl IntroSection {
    /// FORMAT.md §3.6
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        expect_magic_u16(reader, "intro", INTRO_MAGIC)?;
        let declared_bytes = reader.read_u16()?;
        if usize::from(declared_bytes) != INTRO_BYTES {
            return Err(FormatError::BadSectionSize {
                section: "intro",
                expected: INTRO_BYTES,
                found: usize::from(declared_bytes),
            });
        }
        let mut quest_intro = [[0u8; 8]; 3];
        for slot in &mut quest_intro {
            *slot = reader.read_array::<8>()?;
        }
        let mut npc_intro = [[0u8; 8]; 3];
        for slot in &mut npc_intro {
            *slot = reader.read_array::<8>()?;
        }
        Ok(Self {
            declared_bytes,
            quest_intro,
            npc_intro,
        })
    }

    /// FORMAT.md §3.6
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u16(INTRO_MAGIC)?;
        writer.write_u16(self.declared_bytes)?;
        for flags in &self.quest_intro {
            writer.write_bytes(flags)?;
        }
        for flags in &self.npc_intro {
            writer.write_bytes(flags)?;
        }
        Ok(())
    }
}

/// FORMAT.md §3.8
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsSection {
    pub levels: Vec<u8>,
}

impl SkillsSection {
    /// FORMAT.md §3.8
    pub fn read(reader: &mut BitReader<'_>, count: usize) -> Result<Self, FormatError> {
        expect_magic_u16(reader, "skills", SKILLS_MAGIC)?;
        Ok(Self {
            levels: reader.read_bytes(count)?.to_vec(),
        })
    }

    /// FORMAT.md §3.8
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u16(SKILLS_MAGIC)?;
        writer.write_bytes(&self.levels)?;
        Ok(())
    }
}

pub(crate) fn read_stats_magic(reader: &mut BitReader<'_>) -> Result<(), FormatError> {
    expect_magic_u16(reader, "stats", STATS_MAGIC)
}
