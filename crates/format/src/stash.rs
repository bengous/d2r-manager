use crate::bits::{BitReader, BitWriter};
use crate::error::{FormatError, SUPPORTED_VERSION};
use crate::save::ItemList;

/// FORMAT.md §4.1
pub const TAB_MAGIC: u32 = 0xAA55_AA55;
/// FORMAT.md §4.1
pub const TAB_HEADER_BYTES: usize = 64;
/// FORMAT.md §4.3
pub const CHRONICLE_MAGIC: u32 = 0xC0EA_EDC0;

/// FORMAT.md §4.2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Normal,
    AdvancedStash,
    Chronicle,
}

impl TabKind {
    const fn from_code(code: u8) -> Result<Self, FormatError> {
        Ok(match code {
            0 => Self::Normal,
            1 => Self::AdvancedStash,
            2 => Self::Chronicle,
            _ => return Err(FormatError::UnknownStashTabType { kind: code }),
        })
    }

    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// FORMAT.md §4.3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChronicleEntry {
    pub item_id: u32,
    pub source: u16,
    pub timestamp: u32,
}

/// FORMAT.md §4.3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChronicleSection {
    pub version: u16,
    pub reserved: [u8; 8],
    pub sets: Vec<ChronicleEntry>,
    pub uniques: Vec<ChronicleEntry>,
    pub runewords: Vec<ChronicleEntry>,
}

impl ChronicleSection {
    /// FORMAT.md §4.3
    fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let at = reader.byte_position()?;
        let magic = reader.read_u32()?;
        if magic != CHRONICLE_MAGIC {
            return Err(FormatError::BadMagic {
                section: "chronicle",
                expected: CHRONICLE_MAGIC,
                found: magic,
                at,
            });
        }
        let version = reader.read_u16()?;
        let set_count = reader.read_u16()?;
        let unique_count = reader.read_u16()?;
        let runeword_count = reader.read_u16()?;
        let reserved = reader.read_array::<8>()?;

        let mut read_entries = |count: u16| -> Result<Vec<ChronicleEntry>, FormatError> {
            let mut entries = Vec::with_capacity(usize::from(count));
            for _ in 0..count {
                entries.push(ChronicleEntry {
                    item_id: reader.read_u32()?,
                    source: reader.read_u16()?,
                    timestamp: reader.read_u32()?,
                });
            }
            Ok(entries)
        };

        Ok(Self {
            version,
            reserved,
            sets: read_entries(set_count)?,
            uniques: read_entries(unique_count)?,
            runewords: read_entries(runeword_count)?,
        })
    }

    /// FORMAT.md §4.3
    fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_u32(CHRONICLE_MAGIC)?;
        writer.write_u16(self.version)?;
        for list in [&self.sets, &self.uniques, &self.runewords] {
            writer.write_u16(u16::try_from(list.len()).map_err(|_| {
                FormatError::BadSectionSize {
                    section: "chronicle",
                    expected: usize::from(u16::MAX),
                    found: list.len(),
                }
            })?)?;
        }
        writer.write_bytes(&self.reserved)?;
        for list in [&self.sets, &self.uniques, &self.runewords] {
            for entry in list {
                writer.write_u32(entry.item_id)?;
                writer.write_u16(entry.source)?;
                writer.write_u32(entry.timestamp)?;
            }
        }
        Ok(())
    }
}

/// FORMAT.md §4.2
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabBody {
    Items(ItemList),
    Chronicle(ChronicleSection),
}

/// FORMAT.md §4.1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashTab {
    pub stash_format: u32,
    pub item_format: u32,
    pub gold: u32,
    pub season: u16,
    pub kind: TabKind,
    pub reserved: [u8; 43],
    pub body: TabBody,
    pub trailing: Vec<u8>,
}

/// FORMAT.md §4
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashSave {
    pub tabs: Vec<StashTab>,
    pub trailing: Vec<u8>,
}

impl StashSave {
    /// FORMAT.md §4
    pub fn read(data: &[u8]) -> Result<Self, FormatError> {
        let mut reader = BitReader::new(data);
        let mut tabs = Vec::new();
        while reader.bits_left() > TAB_HEADER_BYTES * 8 {
            tabs.push(StashTab::read(&mut reader, tabs.len())?);
        }
        let left = reader.bits_left() / 8;
        Ok(Self {
            tabs,
            trailing: reader.read_bytes(left)?.to_vec(),
        })
    }

    /// FORMAT.md §4
    pub fn write(&self) -> Result<Vec<u8>, FormatError> {
        let mut writer = BitWriter::new();
        for tab in &self.tabs {
            tab.write(&mut writer)?;
        }
        if !self.trailing.is_empty() {
            writer.write_bytes(&self.trailing)?;
        }
        Ok(writer.finish())
    }
}

impl StashTab {
    /// FORMAT.md §4.1
    fn read(reader: &mut BitReader<'_>, index: usize) -> Result<Self, FormatError> {
        let start = reader.byte_position()?;
        let magic = reader.read_u32()?;
        if magic != TAB_MAGIC {
            return Err(FormatError::BadMagic {
                section: "stash tab",
                expected: TAB_MAGIC,
                found: magic,
                at: start,
            });
        }
        let stash_format = reader.read_u32()?;
        let item_format = reader.read_u32()?;
        if item_format != SUPPORTED_VERSION {
            return Err(FormatError::UnsupportedVersion {
                version: item_format,
            });
        }
        let gold = reader.read_u32()?;
        let declared = usize::from(reader.read_u16()?);
        let season = reader.read_u16()?;
        let kind = TabKind::from_code(reader.read_u8()?)?;
        let reserved = reader.read_array::<43>()?;

        if declared < TAB_HEADER_BYTES {
            return Err(FormatError::StashTabTooShort {
                tab: index,
                declared,
                minimum: TAB_HEADER_BYTES,
            });
        }

        let body = match kind {
            TabKind::Chronicle => TabBody::Chronicle(ChronicleSection::read(reader)?),
            TabKind::Normal | TabKind::AdvancedStash => TabBody::Items(ItemList::read(reader)?),
        };

        let consumed = reader.byte_position()? - start;
        if consumed > declared {
            return Err(FormatError::StashTabOverrun {
                tab: index,
                declared,
                overrun: consumed - declared,
            });
        }
        let trailing = reader.read_bytes(declared - consumed)?.to_vec();

        Ok(Self {
            stash_format,
            item_format,
            gold,
            season,
            kind,
            reserved,
            body,
            trailing,
        })
    }

    /// FORMAT.md §4.1
    fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        let start = writer.byte_len();
        writer.write_u32(TAB_MAGIC)?;
        writer.write_u32(self.stash_format)?;
        writer.write_u32(self.item_format)?;
        writer.write_u32(self.gold)?;
        let size_field = writer.byte_len();
        writer.write_u16(0)?;
        writer.write_u16(self.season)?;
        writer.write_u8(self.kind.code())?;
        writer.write_bytes(&self.reserved)?;

        match &self.body {
            TabBody::Items(items) => items.write(writer)?,
            TabBody::Chronicle(chronicle) => chronicle.write(writer)?,
        }
        if !self.trailing.is_empty() {
            writer.write_bytes(&self.trailing)?;
        }

        let size = writer.byte_len() - start;
        writer.patch_u16(
            size_field,
            u16::try_from(size).map_err(|_| FormatError::BadSectionSize {
                section: "stash tab",
                expected: usize::from(u16::MAX),
                found: size,
            })?,
        );
        Ok(())
    }
}
