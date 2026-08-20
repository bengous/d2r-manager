use crate::bits::{BitReader, BitWriter, Bits};
use crate::error::FormatError;
use crate::text::FixedString;

/// FORMAT.md §3.2
pub const SECTION_BYTES: usize = 387;

/// FORMAT.md §3.2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterClass {
    Amazon,
    Sorceress,
    Necromancer,
    Paladin,
    Barbarian,
    Druid,
    Assassin,
    Warlock,
}

impl CharacterClass {
    const fn from_code(code: u8) -> Result<Self, FormatError> {
        Ok(match code {
            0 => Self::Amazon,
            1 => Self::Sorceress,
            2 => Self::Necromancer,
            3 => Self::Paladin,
            4 => Self::Barbarian,
            5 => Self::Druid,
            6 => Self::Assassin,
            7 => Self::Warlock,
            _ => return Err(FormatError::UnknownCharacterClass { class: code }),
        })
    }

    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// FORMAT.md §3.3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameVersion {
    None,
    Classic,
    Expansion,
    ReignOfTheWarlock,
}

impl GameVersion {
    const fn from_code(code: u8) -> Result<Self, FormatError> {
        Ok(match code {
            0 => Self::None,
            1 => Self::Classic,
            2 => Self::Expansion,
            3 => Self::ReignOfTheWarlock,
            _ => return Err(FormatError::UnknownGameVersion { version: code }),
        })
    }

    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn is_expansion(self) -> bool {
        matches!(self, Self::Expansion | Self::ReignOfTheWarlock)
    }
}

/// FORMAT.md §3.2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSide {
    Right,
    Left,
}

/// FORMAT.md §3.2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Skill {
    pub id: Bits<15>,
    pub side: SkillSide,
    pub item_index: u16,
}

impl Skill {
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let id = reader.read::<15>()?;
        let side = if reader.read_bool()? {
            SkillSide::Left
        } else {
            SkillSide::Right
        };
        Ok(Self {
            id,
            side,
            item_index: reader.read_u16()?,
        })
    }

    fn write(self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write(self.id)?;
        writer.write_bool(matches!(self.side, SkillSide::Left))?;
        writer.write_u16(self.item_index)?;
        Ok(())
    }
}

/// FORMAT.md §3.2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MercData {
    pub flags: u32,
    pub seed: u32,
    pub name_index: u16,
    pub hireling_id: u16,
    pub experience: u32,
}

impl MercData {
    #[must_use]
    pub const fn is_hired(self) -> bool {
        self.seed != 0 || self.name_index != 0 || self.experience != 0
    }

    #[must_use]
    pub const fn is_dead(self) -> bool {
        self.flags & 0x0001_0000 != 0
    }

    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        Ok(Self {
            flags: reader.read_u32()?,
            seed: reader.read_u32()?,
            name_index: reader.read_u16()?,
            hireling_id: reader.read_u16()?,
            experience: reader.read_u32()?,
        })
    }

    fn write(self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u32(self.flags)?;
        writer.write_u32(self.seed)?;
        writer.write_u16(self.name_index)?;
        writer.write_u16(self.hireling_id)?;
        writer.write_u32(self.experience)?;
        Ok(())
    }
}

/// FORMAT.md §3.3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewItem {
    pub code: u32,
    pub transform: u8,
    pub quality: u8,
    pub file_index: u16,
    pub flags: u32,
}

impl PreviewItem {
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        Ok(Self {
            code: reader.read_u32()?,
            transform: reader.read_u8()?,
            quality: reader.read_u8()?,
            file_index: reader.read_u16()?,
            flags: reader.read_u32()?,
        })
    }

    fn write(self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u32(self.code)?;
        writer.write_u8(self.transform)?;
        writer.write_u8(self.quality)?;
        writer.write_u16(self.file_index)?;
        writer.write_u32(self.flags)?;
        Ok(())
    }
}

/// FORMAT.md §3.3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewData {
    pub save_times: [u64; 6],
    pub experiences: [u32; 6],
    pub guild_emblem_color: u8,
    pub game_version: GameVersion,
    pub padding: u16,
    pub left_hand: PreviewItem,
    pub right_hand: PreviewItem,
    pub torso: PreviewItem,
    pub head: PreviewItem,
    pub name: FixedString<96>,
    pub reserved: u64,
}

impl PreviewData {
    /// FORMAT.md §3.3
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let mut save_times = [0u64; 6];
        for slot in &mut save_times {
            *slot = reader.read_u64()?;
        }
        let mut experiences = [0u32; 6];
        for slot in &mut experiences {
            *slot = reader.read_u32()?;
        }
        Ok(Self {
            save_times,
            experiences,
            guild_emblem_color: reader.read_u8()?,
            game_version: GameVersion::from_code(reader.read_u8()?)?,
            padding: reader.read_u16()?,
            left_hand: PreviewItem::read(reader)?,
            right_hand: PreviewItem::read(reader)?,
            torso: PreviewItem::read(reader)?,
            head: PreviewItem::read(reader)?,
            name: FixedString::<96>::read(reader)?,
            reserved: reader.read_u64()?,
        })
    }

    /// FORMAT.md §3.3
    fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        for value in self.save_times {
            writer.write_u64(value)?;
        }
        for value in self.experiences {
            writer.write_u32(value)?;
        }
        writer.write_u8(self.guild_emblem_color)?;
        writer.write_u8(self.game_version.code())?;
        writer.write_u16(self.padding)?;
        self.left_hand.write(writer)?;
        self.right_hand.write(writer)?;
        self.torso.write(writer)?;
        self.head.write(writer)?;
        self.name.write(writer)?;
        writer.write_u64(self.reserved)?;
        Ok(())
    }
}

/// FORMAT.md §3.2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Character {
    pub weapon_switch: u32,
    pub flags: u32,
    pub class: CharacterClass,
    pub stat_count: u8,
    pub skill_count: u8,
    pub level: u8,
    pub create_time: u32,
    pub last_save_time: u32,
    pub play_time: u32,
    pub hotkeys: [Skill; 16],
    pub left_skill: Skill,
    pub right_skill: Skill,
    pub switch_left_skill: Skill,
    pub switch_right_skill: Skill,
    pub appearance_graphics: [u8; 16],
    pub appearance_tints: [u8; 16],
    pub town_difficulty: [u8; 3],
    pub map_seed: u32,
    pub merc: MercData,
    pub preview: PreviewData,
}

impl Character {
    /// FORMAT.md §3.2
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let weapon_switch = reader.read_u32()?;
        let flags = reader.read_u32()?;
        let class = CharacterClass::from_code(reader.read_u8()?)?;
        let stat_count = reader.read_u8()?;
        let skill_count = reader.read_u8()?;
        let level = reader.read_u8()?;
        let create_time = reader.read_u32()?;
        let last_save_time = reader.read_u32()?;
        let play_time = reader.read_u32()?;

        let mut hotkeys = [Skill {
            id: Bits::<15>::ZERO,
            side: SkillSide::Right,
            item_index: 0,
        }; 16];
        for slot in &mut hotkeys {
            *slot = Skill::read(reader)?;
        }

        Ok(Self {
            weapon_switch,
            flags,
            class,
            stat_count,
            skill_count,
            level,
            create_time,
            last_save_time,
            play_time,
            hotkeys,
            left_skill: Skill::read(reader)?,
            right_skill: Skill::read(reader)?,
            switch_left_skill: Skill::read(reader)?,
            switch_right_skill: Skill::read(reader)?,
            appearance_graphics: reader.read_array::<16>()?,
            appearance_tints: reader.read_array::<16>()?,
            town_difficulty: reader.read_array::<3>()?,
            map_seed: reader.read_u32()?,
            merc: MercData::read(reader)?,
            preview: PreviewData::read(reader)?,
        })
    }

    /// FORMAT.md §3.2
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u32(self.weapon_switch)?;
        writer.write_u32(self.flags)?;
        writer.write_u8(self.class.code())?;
        writer.write_u8(self.stat_count)?;
        writer.write_u8(self.skill_count)?;
        writer.write_u8(self.level)?;
        writer.write_u32(self.create_time)?;
        writer.write_u32(self.last_save_time)?;
        writer.write_u32(self.play_time)?;
        for hotkey in self.hotkeys {
            hotkey.write(writer)?;
        }
        self.left_skill.write(writer)?;
        self.right_skill.write(writer)?;
        self.switch_left_skill.write(writer)?;
        self.switch_right_skill.write(writer)?;
        writer.write_bytes(&self.appearance_graphics)?;
        writer.write_bytes(&self.appearance_tints)?;
        writer.write_bytes(&self.town_difficulty)?;
        writer.write_u32(self.map_seed)?;
        self.merc.write(writer)?;
        self.preview.write(writer)
    }
}
