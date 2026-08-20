use crate::bits::{BitReader, BitWriter};
use crate::error::FormatError;
use crate::tables::stat_info;

/// FORMAT.md §5.8
pub const TERMINATOR: u16 = 0x1FF;

const ID_BITS: u32 = 9;

/// FORMAT.md §5.8
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StatId(u16);

impl StatId {
    pub const MAX_DAMAGE_PERCENT: Self = Self(17);
    pub const FIRE_MIN_DAMAGE: Self = Self(48);
    pub const LIGHTNING_MIN_DAMAGE: Self = Self(50);
    pub const MAGIC_MIN_DAMAGE: Self = Self(52);
    pub const COLD_MIN_DAMAGE: Self = Self(54);
    pub const POISON_MIN_DAMAGE: Self = Self(57);
    pub const POISON_COUNT: Self = Self(326);
    pub const ARMOR_CLASS: Self = Self(31);
    pub const DURABILITY: Self = Self(72);
    pub const MAX_DURABILITY: Self = Self(73);
    pub const NUM_SOCKETS: Self = Self(194);
    pub const QUEST_ITEM_DIFFICULTY: Self = Self(356);

    pub const fn new(id: u16) -> Result<Self, FormatError> {
        if id == TERMINATOR {
            return Err(FormatError::StatIdIsTerminator { id });
        }
        if id > TERMINATOR {
            return Err(FormatError::UnknownStatId { id });
        }
        Ok(Self(id))
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// FORMAT.md §5.8
    #[must_use]
    pub const fn paired(self) -> &'static [Self] {
        match self.0 {
            17 => &[Self(18)],
            48 => &[Self(49)],
            50 => &[Self(51)],
            52 => &[Self(53)],
            54 => &[Self(55), Self(56)],
            57 => &[Self(58), Self(59)],
            _ => &[],
        }
    }
}

/// FORMAT.md §5.8
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStat {
    pub id: StatId,
    pub param: u32,
    pub value: i64,
    pub paired: Vec<i64>,
}

/// FORMAT.md §5.8
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemStats(pub Vec<ItemStat>);

fn decode(raw: u32, save_add: i32, val_shift: u32) -> i64 {
    (i64::from(raw) - i64::from(save_add)) << val_shift
}

pub(crate) fn encode_value(
    id: StatId,
    value: i64,
    save_add: i32,
    val_shift: u32,
    bits: u32,
) -> Result<u32, FormatError> {
    let raw = (value >> val_shift) + i64::from(save_add);
    u32::try_from(raw)
        .ok()
        .filter(|raw| bits == 32 || u64::from(*raw) < 1u64 << bits)
        .ok_or(FormatError::StatValueOutOfRange {
            id: id.get(),
            value,
            bits,
        })
}

fn narrow_unsigned(id: StatId, value: i64, bits: u32) -> Result<u32, FormatError> {
    u32::try_from(value)
        .ok()
        .filter(|raw| bits >= 32 || u64::from(*raw) < 1u64 << bits)
        .ok_or(FormatError::StatValueOutOfRange {
            id: id.get(),
            value,
            bits,
        })
}

fn narrow_signed(id: StatId, value: i64, bits: u32) -> Result<i32, FormatError> {
    i32::try_from(value).map_err(|_| FormatError::StatValueOutOfRange {
        id: id.get(),
        value,
        bits,
    })
}

impl ItemStats {
    /// FORMAT.md §5.8
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let mut stats = Vec::new();
        loop {
            #[expect(clippy::cast_possible_truncation, reason = "9 bits requested")]
            let raw_id = reader.read_bits(ID_BITS)? as u16;
            if raw_id == TERMINATOR {
                return Ok(Self(stats));
            }
            let id = StatId::new(raw_id)?;
            let info = stat_info(raw_id)?;

            let param = reader.read_bits(info.save_param_bits)?;
            let value = decode(
                reader.read_bits(info.save_bits)?,
                info.save_add,
                info.val_shift,
            );

            let mut paired = Vec::with_capacity(id.paired().len());
            for follower in id.paired() {
                let info = stat_info(follower.get())?;
                paired.push(decode(
                    reader.read_bits(info.save_bits)?,
                    info.save_add,
                    info.val_shift,
                ));
            }

            stats.push(ItemStat {
                id,
                param,
                value,
                paired,
            });
        }
    }

    /// FORMAT.md §5.8
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        for stat in &self.0 {
            let info = stat_info(stat.id.get())?;
            writer.write_bits(u32::from(stat.id.get()), ID_BITS)?;
            writer.write_bits(stat.param, info.save_param_bits)?;
            writer.write_bits(
                encode_value(
                    stat.id,
                    stat.value,
                    info.save_add,
                    info.val_shift,
                    info.save_bits,
                )?,
                info.save_bits,
            )?;

            for (follower, value) in stat.id.paired().iter().zip(&stat.paired) {
                let info = stat_info(follower.get())?;
                writer.write_bits(
                    encode_value(
                        *follower,
                        *value,
                        info.save_add,
                        info.val_shift,
                        info.save_bits,
                    )?,
                    info.save_bits,
                )?;
            }
        }
        writer.write_bits(u32::from(TERMINATOR), ID_BITS)?;
        Ok(())
    }
}

/// FORMAT.md §3.7
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerStat {
    pub id: StatId,
    pub layer: i32,
    pub value: i64,
}

/// FORMAT.md §3.7
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerStats(pub Vec<PlayerStat>);

impl PlayerStats {
    /// FORMAT.md §3.7
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let mut stats = Vec::new();
        loop {
            #[expect(clippy::cast_possible_truncation, reason = "9 bits requested")]
            let raw_id = reader.read_bits(ID_BITS)? as u16;
            if raw_id == TERMINATOR {
                return Ok(Self(stats));
            }
            let id = StatId::new(raw_id)?;
            let info = stat_info(raw_id)?;

            let layer = if info.csv_param > 0 {
                reader.read_signed_bits(info.csv_param)?
            } else {
                0
            };
            let value = if info.csv_bits < 32 && info.csv_signed {
                i64::from(reader.read_signed_bits(info.csv_bits)?)
            } else {
                i64::from(reader.read_bits(info.csv_bits)?)
            };

            stats.push(PlayerStat { id, layer, value });
        }
    }

    /// FORMAT.md §3.7
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        for stat in &self.0 {
            let info = stat_info(stat.id.get())?;
            writer.write_bits(u32::from(stat.id.get()), ID_BITS)?;
            if info.csv_param > 0 {
                writer.write_signed_bits(stat.layer, info.csv_param)?;
            }
            if info.csv_bits < 32 && info.csv_signed {
                writer.write_signed_bits(
                    narrow_signed(stat.id, stat.value, info.csv_bits)?,
                    info.csv_bits,
                )?;
            } else {
                writer.write_bits(
                    narrow_unsigned(stat.id, stat.value, info.csv_bits)?,
                    info.csv_bits,
                )?;
            }
        }
        writer.write_bits(u32::from(TERMINATOR), ID_BITS)?;
        Ok(())
    }
}

/// FORMAT.md §3.13
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DemonStats(pub Vec<PlayerStat>);

impl DemonStats {
    /// FORMAT.md §3.13
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let mut stats = Vec::new();
        loop {
            #[expect(clippy::cast_possible_truncation, reason = "9 bits requested")]
            let raw_id = reader.read_bits(ID_BITS)? as u16;
            if raw_id == TERMINATOR {
                return Ok(Self(stats));
            }
            let id = StatId::new(raw_id)?;
            let info = stat_info(raw_id)?;
            let layer = if info.csv_param > 0 {
                reader.read_signed_bits(info.csv_param)?
            } else {
                0
            };
            stats.push(PlayerStat {
                id,
                layer,
                value: i64::from(reader.read_bits(32)?),
            });
        }
    }

    /// FORMAT.md §3.13
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        for stat in &self.0 {
            let info = stat_info(stat.id.get())?;
            writer.write_bits(u32::from(stat.id.get()), ID_BITS)?;
            if info.csv_param > 0 {
                writer.write_signed_bits(stat.layer, info.csv_param)?;
            }
            writer.write_bits(narrow_unsigned(stat.id, stat.value, 32)?, 32)?;
        }
        writer.write_bits(u32::from(TERMINATOR), ID_BITS)?;
        Ok(())
    }
}
