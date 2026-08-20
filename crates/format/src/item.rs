use crate::bits::{BitReader, BitWriter, Bits};
use crate::code::ItemCode;
use crate::error::FormatError;
use crate::huffman;
use crate::stats::{ItemStats, StatId, encode_value};
use crate::tables::{ItemInfo, item_info, stat_info};
use crate::text::SaveString;

/// FORMAT.md §5.2
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemFlags(u32);

impl ItemFlags {
    pub const NEW_ITEM: u32 = 0x0000_0001;
    pub const TARGET: u32 = 0x0000_0002;
    pub const TARGETING: u32 = 0x0000_0004;
    pub const DELETED: u32 = 0x0000_0008;
    pub const IDENTIFIED: u32 = 0x0000_0010;
    pub const HAS_QUANTITY: u32 = 0x0000_0020;
    pub const SWITCH_IN: u32 = 0x0000_0040;
    pub const SWITCH_OUT: u32 = 0x0000_0080;
    pub const BROKEN: u32 = 0x0000_0100;
    pub const REPAIRED: u32 = 0x0000_0200;
    pub const SOCKETED: u32 = 0x0000_0800;
    pub const NO_SELL: u32 = 0x0000_1000;
    pub const IN_STORE: u32 = 0x0000_2000;
    pub const NO_EQUIP: u32 = 0x0000_4000;
    pub const NAMED: u32 = 0x0000_8000;
    pub const IS_EAR: u32 = 0x0001_0000;
    pub const STARTER_ITEM: u32 = 0x0002_0000;
    pub const INIT: u32 = 0x0008_0000;
    pub const COMPACT_SAVE: u32 = 0x0020_0000;
    pub const ETHEREAL: u32 = 0x0040_0000;
    pub const JUST_SAVED: u32 = 0x0080_0000;
    pub const PERSONALIZED: u32 = 0x0100_0000;
    pub const LOW_QUALITY: u32 = 0x0200_0000;
    pub const RUNEWORD: u32 = 0x0400_0000;
    pub const DUPLICATED: u32 = 0x0800_0000;
    pub const CHRONICLE: u32 = 0x1000_0000;
    pub const CHRONICLE_COMPACT: u32 = 0x2000_0000;

    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn has(self, flag: u32) -> bool {
        self.0 & flag != 0
    }
}

/// FORMAT.md §5.3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemVersion {
    above_ninety_nine: bool,
    offset: Bits<2>,
}

impl ItemVersion {
    /// FORMAT.md §5.3
    pub const fn from_number(number: u16) -> Result<Self, FormatError> {
        let (above_ninety_nine, offset) = match number {
            0..=3 => (false, number),
            99..=102 => (true, number - 99),
            _ => return Err(FormatError::UnknownItemVersion { version: number }),
        };
        match Bits::<2>::new(offset as u32) {
            Some(offset) => Ok(Self {
                above_ninety_nine,
                offset,
            }),
            None => Err(FormatError::UnknownItemVersion { version: number }),
        }
    }

    #[must_use]
    pub const fn number(self) -> u16 {
        #[expect(clippy::cast_possible_truncation, reason = "offset holds 2 bits")]
        let offset = self.offset.get() as u16;
        if self.above_ninety_nine {
            offset + 99
        } else {
            offset
        }
    }

    /// FORMAT.md §5.3
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        Ok(Self {
            above_ninety_nine: reader.read_bool()?,
            offset: reader.read::<2>()?,
        })
    }

    /// FORMAT.md §5.3
    fn write(self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_bool(self.above_ninety_nine)?;
        writer.write(self.offset)?;
        Ok(())
    }
}

/// FORMAT.md §5.5
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCell {
    column: Bits<4>,
    row: Bits<4>,
}

impl GridCell {
    pub fn new(column: u32, row: u32) -> Result<Self, FormatError> {
        let cell = Bits::<4>::new(column).zip(Bits::<4>::new(row));
        cell.map(|(column, row)| Self { column, row })
            .ok_or(FormatError::GridCellOutOfRange { column, row })
    }

    #[must_use]
    pub const fn column(self) -> u32 {
        self.column.get()
    }

    #[must_use]
    pub const fn row(self) -> u32 {
        self.row.get()
    }
}

/// FORMAT.md §5.5
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotMode {
    Stored,
    Equipped,
    InBelt,
    OnCursor,
    Socketed,
}

/// FORMAT.md §5.5
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundMode {
    OnGround,
    Dropping,
}

/// FORMAT.md §5.5
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyLocation {
    None,
    Head,
    Neck,
    Torso,
    RightArm,
    LeftArm,
    RightRing,
    LeftRing,
    Belt,
    Feet,
    Gloves,
    RightHand,
    LeftHand,
}

/// FORMAT.md §5.5
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorePage {
    None,
    Inventory,
    Equip,
    Trade,
    Cube,
    Stash,
    Belt,
    Seventh,
}

/// FORMAT.md §5.5
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemPosition {
    Ground {
        mode: GroundMode,
        x: u16,
        y: u16,
    },
    Slot {
        mode: SlotMode,
        body: BodyLocation,
        cell: GridCell,
        page: StorePage,
    },
}

impl SlotMode {
    const fn code(self) -> u32 {
        match self {
            Self::Stored => 0,
            Self::Equipped => 1,
            Self::InBelt => 2,
            Self::OnCursor => 4,
            Self::Socketed => 6,
        }
    }
}

impl GroundMode {
    const fn code(self) -> u32 {
        match self {
            Self::OnGround => 3,
            Self::Dropping => 5,
        }
    }
}

impl BodyLocation {
    const fn from_code(code: u32) -> Result<Self, FormatError> {
        Ok(match code {
            0 => Self::None,
            1 => Self::Head,
            2 => Self::Neck,
            3 => Self::Torso,
            4 => Self::RightArm,
            5 => Self::LeftArm,
            6 => Self::RightRing,
            7 => Self::LeftRing,
            8 => Self::Belt,
            9 => Self::Feet,
            10 => Self::Gloves,
            11 => Self::RightHand,
            12 => Self::LeftHand,
            _ => return Err(FormatError::UnknownBodyLocation { location: code }),
        })
    }

    #[must_use]
    pub const fn code(self) -> u32 {
        self as u32
    }
}

impl StorePage {
    const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Inventory,
            2 => Self::Equip,
            3 => Self::Trade,
            4 => Self::Cube,
            5 => Self::Stash,
            6 => Self::Belt,
            7 => Self::Seventh,
            _ => Self::None,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u32 {
        self as u32
    }
}

impl ItemPosition {
    /// FORMAT.md §5.5
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let mode = reader.read::<3>()?.get();
        if let 3 | 5 = mode {
            return Ok(Self::Ground {
                mode: if mode == 3 {
                    GroundMode::OnGround
                } else {
                    GroundMode::Dropping
                },
                x: reader.read_u16()?,
                y: reader.read_u16()?,
            });
        }
        let mode = match mode {
            0 => SlotMode::Stored,
            1 => SlotMode::Equipped,
            2 => SlotMode::InBelt,
            4 => SlotMode::OnCursor,
            6 => SlotMode::Socketed,
            other => return Err(FormatError::UnknownItemMode { mode: other }),
        };
        let body = BodyLocation::from_code(reader.read::<4>()?.get())?;
        let column = reader.read::<4>()?.get();
        let row = reader.read::<4>()?.get();
        Ok(Self::Slot {
            mode,
            body,
            cell: GridCell::new(column, row)?,
            page: StorePage::from_raw(reader.read::<3>()?.get()),
        })
    }

    /// FORMAT.md §5.5
    fn write(self, writer: &mut BitWriter) -> Result<(), FormatError> {
        match self {
            Self::Ground { mode, x, y } => {
                writer.write_bits(mode.code(), 3)?;
                writer.write_u16(x)?;
                writer.write_u16(y)?;
            }
            Self::Slot {
                mode,
                body,
                cell,
                page,
            } => {
                writer.write_bits(mode.code(), 3)?;
                writer.write_bits(body.code(), 4)?;
                writer.write(cell.column)?;
                writer.write(cell.row)?;
                writer.write_bits(page.raw(), 3)?;
            }
        }
        Ok(())
    }
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoldAmount {
    Small(Bits<12>),
    Large(u32),
}

impl GoldAmount {
    #[must_use]
    pub const fn get(self) -> u32 {
        match self {
            Self::Small(amount) => amount.get(),
            Self::Large(amount) => amount,
        }
    }

    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        if reader.read_bool()? {
            Ok(Self::Large(reader.read_u32()?))
        } else {
            Ok(Self::Small(reader.read::<12>()?))
        }
    }

    fn write(self, writer: &mut BitWriter) -> Result<(), FormatError> {
        match self {
            Self::Small(amount) => {
                writer.write_bool(false)?;
                writer.write(amount)?;
            }
            Self::Large(amount) => {
                writer.write_bool(true)?;
                writer.write_u32(amount)?;
            }
        }
        Ok(())
    }
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gold {
    pub amount: GoldAmount,
    pub player_gold: bool,
}

impl Gold {
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        Ok(Self {
            amount: GoldAmount::read(reader)?,
            player_gold: reader.read_bool()?,
        })
    }

    fn write(self, writer: &mut BitWriter) -> Result<(), FormatError> {
        self.amount.write(writer)?;
        writer.write_bool(self.player_gold)?;
        Ok(())
    }
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarData {
    pub file_index: Bits<3>,
    pub level: Bits<7>,
    pub name: SaveString,
}

impl EarData {
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        Ok(Self {
            file_index: reader.read::<3>()?,
            level: reader.read::<7>()?,
            name: SaveString::read(reader)?,
        })
    }

    fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write(self.file_index)?;
        writer.write(self.level)?;
        self.name.write(writer)
    }
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemName {
    Absent,
    Ear(EarData),
    Personalized(SaveString),
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterIdentity {
    pub game_account_id: u32,
    pub character_id: u32,
}

/// FORMAT.md §5.9
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChronicleEntries {
    Compact(CharacterIdentity),
    Dated {
        timestamp: u32,
        recipients: Vec<CharacterIdentity>,
    },
}

/// FORMAT.md §5.9
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemChronicle {
    pub id: Bits<16>,
    pub entries: ChronicleEntries,
}

/// FORMAT.md §5.7
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagicAffix {
    Prefix(Bits<11>),
    Suffix(Bits<11>),
}

/// FORMAT.md §5.7
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AffixSlot {
    pub prefix: Option<Bits<11>>,
    pub suffix: Option<Bits<11>>,
}

/// FORMAT.md §5.7
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RareNames {
    pub prefix: Bits<8>,
    pub suffix: Bits<8>,
    pub affixes: [AffixSlot; 3],
}

/// FORMAT.md §5.7
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalExtra {
    Absent,
    Charm(MagicAffix),
    BodyPart(Bits<10>),
    ScrollOrBook(Bits<5>),
}

/// FORMAT.md §5.7
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetBonuses {
    pub mask: Bits<5>,
    pub lists: Vec<ItemStats>,
}

/// FORMAT.md §5.7
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemQuality {
    Inferior(Bits<3>),
    Normal(NormalExtra),
    Superior(Bits<3>),
    Magic {
        prefix: Bits<11>,
        suffix: Bits<11>,
    },
    Set {
        file_index: Bits<12>,
        bonuses: SetBonuses,
    },
    Rare(RareNames),
    Unique(Bits<12>),
    Craft(RareNames),
    Tempered {
        prefix: Bits<8>,
        suffix: Bits<8>,
    },
}

impl ItemQuality {
    #[must_use]
    pub const fn code(&self) -> u32 {
        match self {
            Self::Inferior(_) => 1,
            Self::Normal(_) => 2,
            Self::Superior(_) => 3,
            Self::Magic { .. } => 4,
            Self::Set { .. } => 5,
            Self::Rare(_) => 6,
            Self::Unique(_) => 7,
            Self::Craft(_) => 8,
            Self::Tempered { .. } => 9,
        }
    }
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    Indestructible,
    Limited { max: i64, current: i64 },
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSpecific {
    Absent,
    Armor {
        defense: i64,
        durability: Durability,
    },
    Weapon(Durability),
    Gold(Gold),
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runeword {
    pub id: Bits<16>,
    pub stats: ItemStats,
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sockets {
    pub filled: Vec<Item>,
    pub capacity: Option<u32>,
}

/// FORMAT.md §5.6
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteBody {
    pub code: ItemCode,
    pub seed: u32,
    pub level: Bits<7>,
    pub quality: ItemQuality,
    pub variable_gfx: Option<Bits<3>>,
    pub auto_affix: Option<Bits<11>>,
    pub runeword: Option<Runeword>,
    pub name: ItemName,
    pub realm: Option<[u32; 4]>,
    pub type_specific: TypeSpecific,
    pub quantity: Option<Bits<9>>,
    pub sockets: Sockets,
    pub stats: ItemStats,
    pub chronicle: Option<ItemChronicle>,
    pub advanced_stash_stack: Option<Bits<8>>,
}

/// FORMAT.md §5.5
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactContent {
    Ear(EarData),
    Code { code: ItemCode, gold: Option<Gold> },
}

/// FORMAT.md §5.5
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactBody {
    pub content: CompactContent,
    pub quest_difficulty: Option<i64>,
    pub realm: Option<[u32; 4]>,
    pub advanced_stash_stack: Option<Bits<8>>,
}

/// FORMAT.md §5.1
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemBody {
    Compact(CompactBody),
    Complete(Box<CompleteBody>),
}

/// FORMAT.md §5.1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub flags: ItemFlags,
    pub version: ItemVersion,
    pub position: ItemPosition,
    pub body: ItemBody,
}

impl Item {
    #[must_use]
    pub const fn code(&self) -> Option<ItemCode> {
        match &self.body {
            ItemBody::Compact(body) => match &body.content {
                CompactContent::Ear(_) => None,
                CompactContent::Code { code, .. } => Some(*code),
            },
            ItemBody::Complete(body) => Some(body.code),
        }
    }

    #[must_use]
    pub fn sockets(&self) -> &[Item] {
        match &self.body {
            ItemBody::Compact(_) => &[],
            ItemBody::Complete(body) => &body.sockets.filled,
        }
    }

    /// FORMAT.md §5.1
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let flags = ItemFlags::from_bits(reader.read_u32()?);
        if flags.has(ItemFlags::LOW_QUALITY) {
            return Err(FormatError::GambleItem);
        }
        let version = ItemVersion::read(reader)?;
        let position = ItemPosition::read(reader)?;

        let (mut body, pending) = if flags.has(ItemFlags::COMPACT_SAVE) {
            (ItemBody::Compact(read_compact(reader, flags)?), 0)
        } else {
            let (complete, pending) = read_complete(reader, flags)?;
            (ItemBody::Complete(Box::new(complete)), pending)
        };

        reader.align();

        if let ItemBody::Complete(complete) = &mut body {
            for _ in 0..pending {
                complete.sockets.filled.push(Self::read(reader)?);
            }
        }

        Ok(Self {
            flags,
            version,
            position,
            body,
        })
    }

    /// FORMAT.md §5.1
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u32(self.flags.bits())?;
        self.version.write(writer)?;
        self.position.write(writer)?;

        match &self.body {
            ItemBody::Compact(body) => write_compact(writer, self.flags, body)?,
            ItemBody::Complete(body) => write_complete(writer, self.flags, body)?,
        }

        writer.align();

        for socket in self.sockets() {
            socket.write(writer)?;
        }
        Ok(())
    }
}

fn read_realm(reader: &mut BitReader<'_>) -> Result<Option<[u32; 4]>, FormatError> {
    if !reader.read_bool()? {
        return Ok(None);
    }
    let mut realm = [0u32; 4];
    for slot in &mut realm {
        *slot = reader.read_u32()?;
    }
    Ok(Some(realm))
}

fn write_realm(writer: &mut BitWriter, realm: Option<&[u32; 4]>) -> Result<(), FormatError> {
    writer.write_bool(realm.is_some())?;
    for value in realm.into_iter().flatten() {
        writer.write_u32(*value)?;
    }
    Ok(())
}

fn read_optional<const N: u32>(reader: &mut BitReader<'_>) -> Result<Option<Bits<N>>, FormatError> {
    if reader.read_bool()? {
        Ok(Some(reader.read::<N>()?))
    } else {
        Ok(None)
    }
}

fn write_optional<const N: u32>(
    writer: &mut BitWriter,
    value: Option<Bits<N>>,
) -> Result<(), FormatError> {
    writer.write_bool(value.is_some())?;
    if let Some(value) = value {
        writer.write(value)?;
    }
    Ok(())
}

/// FORMAT.md §5.6
fn read_type_specific(
    reader: &mut BitReader<'_>,
    info: ItemInfo,
) -> Result<TypeSpecific, FormatError> {
    if info.is_armor() {
        let stat = stat_info(StatId::ARMOR_CLASS.get())?;
        let defense = i64::from(reader.read_bits(stat.save_bits)?) - i64::from(stat.save_add);
        return Ok(TypeSpecific::Armor {
            defense,
            durability: read_durability(reader)?,
        });
    }
    if info.is_weapon() {
        return Ok(TypeSpecific::Weapon(read_durability(reader)?));
    }
    if info.is_gold() {
        return Ok(TypeSpecific::Gold(Gold::read(reader)?));
    }
    Ok(TypeSpecific::Absent)
}

/// FORMAT.md §5.5
fn quest_difficulty_info(code: ItemCode) -> Result<Option<(u32, i32, u32)>, FormatError> {
    let info = item_info(code)?;
    if !info.is_quest() || !info.quest_diff_check() {
        return Ok(None);
    }
    let stat = stat_info(StatId::QUEST_ITEM_DIFFICULTY.get())?;
    Ok(Some((stat.save_bits, stat.save_add, stat.val_shift)))
}

/// FORMAT.md §5.5
fn read_compact(reader: &mut BitReader<'_>, flags: ItemFlags) -> Result<CompactBody, FormatError> {
    let content = if flags.has(ItemFlags::IS_EAR) {
        CompactContent::Ear(EarData::read(reader)?)
    } else {
        let code = huffman::decode(reader)?;
        let gold = if item_info(code)?.is_gold() {
            Some(Gold::read(reader)?)
        } else {
            None
        };
        CompactContent::Code { code, gold }
    };

    let code = match &content {
        CompactContent::Ear(_) => ItemCode::EAR,
        CompactContent::Code { code, .. } => *code,
    };

    let quest_difficulty = match quest_difficulty_info(code)? {
        Some((bits, add, shift)) => {
            let raw = reader.read_bits(bits)?;
            Some((i64::from(raw) - i64::from(add)) << shift)
        }
        None => None,
    };

    Ok(CompactBody {
        content,
        quest_difficulty,
        realm: read_realm(reader)?,
        advanced_stash_stack: read_optional::<8>(reader)?,
    })
}

/// FORMAT.md §5.5
fn write_compact(
    writer: &mut BitWriter,
    flags: ItemFlags,
    body: &CompactBody,
) -> Result<(), FormatError> {
    match (&body.content, flags.has(ItemFlags::IS_EAR)) {
        (CompactContent::Ear(ear), true) => ear.write(writer)?,
        (CompactContent::Code { code, gold }, false) => {
            huffman::encode(writer, *code)?;
            if let Some(gold) = gold {
                gold.write(writer)?;
            }
        }
        _ => return Err(FormatError::FlagPayloadMismatch { flag: "IsEar" }),
    }

    let code = match &body.content {
        CompactContent::Ear(_) => ItemCode::EAR,
        CompactContent::Code { code, .. } => *code,
    };

    match (quest_difficulty_info(code)?, body.quest_difficulty) {
        (Some((bits, add, shift)), Some(value)) => {
            let raw = encode_value(StatId::QUEST_ITEM_DIFFICULTY, value, add, shift, bits)?;
            writer.write_bits(raw, bits)?;
        }
        (None, None) => {}
        _ => {
            return Err(FormatError::FlagPayloadMismatch {
                flag: "QuestDiffCheck",
            });
        }
    }

    write_realm(writer, body.realm.as_ref())?;
    write_optional(writer, body.advanced_stash_stack)
}

/// FORMAT.md §5.7
fn read_quality(
    reader: &mut BitReader<'_>,
    code: u32,
    info: ItemInfo,
) -> Result<ItemQuality, FormatError> {
    Ok(match code {
        1 => ItemQuality::Inferior(reader.read::<3>()?),
        2 => ItemQuality::Normal(read_normal_extra(reader, info)?),
        3 => ItemQuality::Superior(reader.read::<3>()?),
        4 => ItemQuality::Magic {
            prefix: reader.read::<11>()?,
            suffix: reader.read::<11>()?,
        },
        5 => ItemQuality::Set {
            file_index: reader.read::<12>()?,
            bonuses: SetBonuses {
                mask: Bits::<5>::ZERO,
                lists: Vec::new(),
            },
        },
        6 => ItemQuality::Rare(read_rare_names(reader)?),
        7 => ItemQuality::Unique(reader.read::<12>()?),
        8 => ItemQuality::Craft(read_rare_names(reader)?),
        9 => ItemQuality::Tempered {
            prefix: reader.read::<8>()?,
            suffix: reader.read::<8>()?,
        },
        other => return Err(FormatError::UnknownItemQuality { quality: other }),
    })
}

/// FORMAT.md §5.7
fn read_normal_extra(
    reader: &mut BitReader<'_>,
    info: ItemInfo,
) -> Result<NormalExtra, FormatError> {
    if info.is_charm() {
        return Ok(NormalExtra::Charm(if reader.read_bool()? {
            MagicAffix::Prefix(reader.read::<11>()?)
        } else {
            MagicAffix::Suffix(reader.read::<11>()?)
        }));
    }
    if info.is_body_part() && !info.is_player_body_part() {
        return Ok(NormalExtra::BodyPart(reader.read::<10>()?));
    }
    if info.is_scroll_or_book() {
        return Ok(NormalExtra::ScrollOrBook(reader.read::<5>()?));
    }
    Ok(NormalExtra::Absent)
}

/// FORMAT.md §5.7
fn read_rare_names(reader: &mut BitReader<'_>) -> Result<RareNames, FormatError> {
    let prefix = reader.read::<8>()?;
    let suffix = reader.read::<8>()?;
    let mut affixes = [AffixSlot::default(); 3];
    for slot in &mut affixes {
        slot.prefix = if reader.read_bool()? {
            Some(reader.read::<11>()?)
        } else {
            None
        };
        slot.suffix = if reader.read_bool()? {
            Some(reader.read::<11>()?)
        } else {
            None
        };
    }
    Ok(RareNames {
        prefix,
        suffix,
        affixes,
    })
}

/// FORMAT.md §5.7
fn write_quality(writer: &mut BitWriter, quality: &ItemQuality) -> Result<(), FormatError> {
    match quality {
        ItemQuality::Inferior(index) | ItemQuality::Superior(index) => writer.write(*index)?,
        ItemQuality::Normal(extra) => match extra {
            NormalExtra::Absent => {}
            NormalExtra::Charm(MagicAffix::Prefix(value)) => {
                writer.write_bool(true)?;
                writer.write(*value)?;
            }
            NormalExtra::Charm(MagicAffix::Suffix(value)) => {
                writer.write_bool(false)?;
                writer.write(*value)?;
            }
            NormalExtra::BodyPart(index) => writer.write(*index)?,
            NormalExtra::ScrollOrBook(spell) => writer.write(*spell)?,
        },
        ItemQuality::Magic { prefix, suffix } => {
            writer.write(*prefix)?;
            writer.write(*suffix)?;
        }
        ItemQuality::Set { file_index, .. } | ItemQuality::Unique(file_index) => {
            writer.write(*file_index)?;
        }
        ItemQuality::Rare(names) | ItemQuality::Craft(names) => {
            writer.write(names.prefix)?;
            writer.write(names.suffix)?;
            for slot in &names.affixes {
                writer.write_bool(slot.prefix.is_some())?;
                if let Some(value) = slot.prefix {
                    writer.write(value)?;
                }
                writer.write_bool(slot.suffix.is_some())?;
                if let Some(value) = slot.suffix {
                    writer.write(value)?;
                }
            }
        }
        ItemQuality::Tempered { prefix, suffix } => {
            writer.write(*prefix)?;
            writer.write(*suffix)?;
        }
    }
    Ok(())
}

/// FORMAT.md §5.6
fn read_durability(reader: &mut BitReader<'_>) -> Result<Durability, FormatError> {
    let max_stat = stat_info(StatId::MAX_DURABILITY.get())?;
    let max = i64::from(reader.read_bits(max_stat.save_bits)?) - i64::from(max_stat.save_add);
    if max <= 0 {
        return Ok(Durability::Indestructible);
    }
    let stat = stat_info(StatId::DURABILITY.get())?;
    let current = i64::from(reader.read_bits(stat.save_bits)?) - i64::from(stat.save_add);
    Ok(Durability::Limited { max, current })
}

/// FORMAT.md §5.6
fn write_durability(writer: &mut BitWriter, durability: Durability) -> Result<(), FormatError> {
    let max_stat = stat_info(StatId::MAX_DURABILITY.get())?;
    let max = match durability {
        Durability::Indestructible => 0,
        Durability::Limited { max, .. } => max,
    };
    writer.write_bits(
        encode_value(
            StatId::MAX_DURABILITY,
            max,
            max_stat.save_add,
            0,
            max_stat.save_bits,
        )?,
        max_stat.save_bits,
    )?;
    if let Durability::Limited { current, .. } = durability {
        let stat = stat_info(StatId::DURABILITY.get())?;
        writer.write_bits(
            encode_value(
                StatId::DURABILITY,
                current,
                stat.save_add,
                0,
                stat.save_bits,
            )?,
            stat.save_bits,
        )?;
    }
    Ok(())
}

/// FORMAT.md §5.6
fn read_complete(
    reader: &mut BitReader<'_>,
    flags: ItemFlags,
) -> Result<(CompleteBody, u32), FormatError> {
    let code = huffman::decode(reader)?;
    let info = item_info(code)?;

    let filled_sockets = reader.read::<3>()?.get();
    let seed = reader.read_u32()?;
    let level = reader.read::<7>()?;
    let quality_code = reader.read::<4>()?.get();

    let variable_gfx = read_optional::<3>(reader)?;
    let auto_affix = read_optional::<11>(reader)?;

    let mut quality = read_quality(reader, quality_code, info)?;

    let runeword_id = if flags.has(ItemFlags::RUNEWORD) {
        Some(reader.read::<16>()?)
    } else {
        None
    };

    let name = if flags.has(ItemFlags::IS_EAR) {
        ItemName::Ear(EarData::read(reader)?)
    } else if flags.has(ItemFlags::PERSONALIZED) {
        ItemName::Personalized(SaveString::read(reader)?)
    } else {
        ItemName::Absent
    };

    let realm = read_realm(reader)?;

    let type_specific = read_type_specific(reader, info)?;
    let quantity = read_optional::<9>(reader)?;

    let capacity = if flags.has(ItemFlags::SOCKETED) {
        let stat = stat_info(StatId::NUM_SOCKETS.get())?;
        Some(reader.read_bits(stat.save_bits)?)
    } else {
        None
    };

    if let ItemQuality::Set { bonuses, .. } = &mut quality {
        bonuses.mask = reader.read::<5>()?;
    }

    let stats = ItemStats::read(reader)?;

    if let ItemQuality::Set { bonuses, .. } = &mut quality {
        for bit in 0..5 {
            if bonuses.mask.get() & (1 << bit) != 0 {
                bonuses.lists.push(ItemStats::read(reader)?);
            }
        }
    }

    let runeword = match runeword_id {
        Some(id) => Some(Runeword {
            id,
            stats: ItemStats::read(reader)?,
        }),
        None => None,
    };

    let chronicle = if flags.has(ItemFlags::CHRONICLE) {
        Some(read_chronicle(reader, flags)?)
    } else {
        None
    };

    Ok((
        CompleteBody {
            code,
            seed,
            level,
            quality,
            variable_gfx,
            auto_affix,
            runeword,
            name,
            realm,
            type_specific,
            quantity,
            sockets: Sockets {
                filled: Vec::new(),
                capacity,
            },
            stats,
            chronicle,
            advanced_stash_stack: read_optional::<8>(reader)?,
        },
        filled_sockets,
    ))
}

/// FORMAT.md §5.9
fn read_chronicle(
    reader: &mut BitReader<'_>,
    flags: ItemFlags,
) -> Result<ItemChronicle, FormatError> {
    let id = reader.read::<16>()?;
    if flags.has(ItemFlags::CHRONICLE_COMPACT) {
        return Ok(ItemChronicle {
            id,
            entries: ChronicleEntries::Compact(read_identity(reader)?),
        });
    }
    let timestamp = reader.read_u32()?;
    let count = reader.read::<4>()?.get();
    let mut recipients = Vec::with_capacity(count as usize);
    for _ in 0..count {
        recipients.push(read_identity(reader)?);
    }
    Ok(ItemChronicle {
        id,
        entries: ChronicleEntries::Dated {
            timestamp,
            recipients,
        },
    })
}

fn read_identity(reader: &mut BitReader<'_>) -> Result<CharacterIdentity, FormatError> {
    Ok(CharacterIdentity {
        game_account_id: reader.read_u32()?,
        character_id: reader.read_u32()?,
    })
}

fn write_identity(writer: &mut BitWriter, identity: CharacterIdentity) -> Result<(), FormatError> {
    writer.write_u32(identity.game_account_id)?;
    writer.write_u32(identity.character_id)?;
    Ok(())
}

/// FORMAT.md §5.9
fn write_chronicle(
    writer: &mut BitWriter,
    flags: ItemFlags,
    chronicle: &ItemChronicle,
) -> Result<(), FormatError> {
    writer.write(chronicle.id)?;
    match (&chronicle.entries, flags.has(ItemFlags::CHRONICLE_COMPACT)) {
        (ChronicleEntries::Compact(identity), true) => write_identity(writer, *identity)?,
        (
            ChronicleEntries::Dated {
                timestamp,
                recipients,
            },
            false,
        ) => {
            writer.write_u32(*timestamp)?;
            let count =
                u32::try_from(recipients.len()).map_err(|_| FormatError::FlagPayloadMismatch {
                    flag: "ChronicleRecipients",
                })?;
            writer.write_bits(count, 4)?;
            for identity in recipients {
                write_identity(writer, *identity)?;
            }
        }
        _ => {
            return Err(FormatError::FlagPayloadMismatch {
                flag: "ChronicleCompact",
            });
        }
    }
    Ok(())
}

/// FORMAT.md §5.6
fn write_complete(
    writer: &mut BitWriter,
    flags: ItemFlags,
    body: &CompleteBody,
) -> Result<(), FormatError> {
    huffman::encode(writer, body.code)?;

    let filled = u32::try_from(body.sockets.filled.len())
        .map_err(|_| FormatError::FlagPayloadMismatch { flag: "Sockets" })?;
    writer.write_bits(filled, 3)?;
    writer.write_u32(body.seed)?;
    writer.write(body.level)?;
    writer.write_bits(body.quality.code(), 4)?;

    write_optional(writer, body.variable_gfx)?;
    write_optional(writer, body.auto_affix)?;

    write_quality(writer, &body.quality)?;

    match (&body.runeword, flags.has(ItemFlags::RUNEWORD)) {
        (Some(runeword), true) => writer.write(runeword.id)?,
        (None, false) => {}
        _ => return Err(FormatError::FlagPayloadMismatch { flag: "Runeword" }),
    }

    match (&body.name, flags.has(ItemFlags::IS_EAR)) {
        (ItemName::Ear(ear), true) => ear.write(writer)?,
        (ItemName::Personalized(name), false) if flags.has(ItemFlags::PERSONALIZED) => {
            name.write(writer)?;
        }
        (ItemName::Absent, false) if !flags.has(ItemFlags::PERSONALIZED) => {}
        _ => return Err(FormatError::FlagPayloadMismatch { flag: "IsEar" }),
    }

    write_realm(writer, body.realm.as_ref())?;

    match body.type_specific {
        TypeSpecific::Absent => {}
        TypeSpecific::Armor {
            defense,
            durability,
        } => {
            let stat = stat_info(StatId::ARMOR_CLASS.get())?;
            writer.write_bits(
                encode_value(
                    StatId::ARMOR_CLASS,
                    defense,
                    stat.save_add,
                    0,
                    stat.save_bits,
                )?,
                stat.save_bits,
            )?;
            write_durability(writer, durability)?;
        }
        TypeSpecific::Weapon(durability) => write_durability(writer, durability)?,
        TypeSpecific::Gold(gold) => gold.write(writer)?,
    }

    write_optional(writer, body.quantity)?;

    match (body.sockets.capacity, flags.has(ItemFlags::SOCKETED)) {
        (Some(capacity), true) => {
            let stat = stat_info(StatId::NUM_SOCKETS.get())?;
            writer.write_bits(capacity, stat.save_bits)?;
        }
        (None, false) => {}
        _ => return Err(FormatError::FlagPayloadMismatch { flag: "Socketed" }),
    }

    if let ItemQuality::Set { bonuses, .. } = &body.quality {
        writer.write(bonuses.mask)?;
    }

    body.stats.write(writer)?;

    if let ItemQuality::Set { bonuses, .. } = &body.quality {
        for list in &bonuses.lists {
            list.write(writer)?;
        }
    }

    if let Some(runeword) = &body.runeword {
        runeword.stats.write(writer)?;
    }

    match (&body.chronicle, flags.has(ItemFlags::CHRONICLE)) {
        (Some(chronicle), true) => write_chronicle(writer, flags, chronicle)?,
        (None, false) => {}
        _ => return Err(FormatError::FlagPayloadMismatch { flag: "Chronicle" }),
    }

    write_optional(writer, body.advanced_stash_stack)
}
