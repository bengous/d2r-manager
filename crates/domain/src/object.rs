use std::fmt;

use format::bits::{BitWriter, Bits};
use format::code::ItemCode;
use format::item::{
    BodyLocation, GridCell, Item, ItemBody, ItemPosition, ItemQuality, SlotMode, StorePage,
};

use crate::error::{ModelError, ValueError};
use crate::tables::{self, CarryGroup, Footprint, UniqueId};

const DIGEST_BYTES: usize = 16;
const FNV_OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fingerprint([u8; DIGEST_BYTES]);

impl Fingerprint {
    pub fn of(item: &Item) -> Result<Self, ModelError> {
        let mut hash = FNV_OFFSET;
        for byte in canonical(item)? {
            hash ^= u128::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        Ok(Self(hash.to_be_bytes()))
    }

    pub fn parse(text: &str) -> Result<Self, ValueError> {
        let expected = DIGEST_BYTES * 2;
        if text.len() != expected {
            return Err(ValueError::FingerprintLength {
                length: text.len(),
                expected,
            });
        }
        let mut digest = [0u8; DIGEST_BYTES];
        for (slot, pair) in digest.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
            let mut value = 0u8;
            for byte in pair {
                let digit = char::from(*byte);
                let nibble = digit
                    .to_digit(16)
                    .ok_or(ValueError::FingerprintDigit { found: digit })?;
                #[expect(clippy::cast_possible_truncation, reason = "a nibble holds 4 bits")]
                let nibble = nibble as u8;
                value = value << 4 | nibble;
            }
            *slot = value;
        }
        Ok(Self(digest))
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; DIGEST_BYTES] {
        self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn canonical(item: &Item) -> Result<Vec<u8>, ModelError> {
    let mut copy = item.clone();
    copy.position = ItemPosition::Slot {
        mode: SlotMode::Stored,
        body: BodyLocation::None,
        cell: GridCell::new(0, 0)?,
        page: StorePage::None,
    };
    let neutral = Some(Bits::<8>::ZERO);
    match &mut copy.body {
        ItemBody::Compact(body) => {
            if body.advanced_stash_stack.is_some() {
                body.advanced_stash_stack = neutral;
            }
        }
        ItemBody::Complete(body) => {
            if body.advanced_stash_stack.is_some() {
                body.advanced_stash_stack = neutral;
            }
        }
    }
    let mut writer = BitWriter::new();
    copy.write(&mut writer)?;
    Ok(writer.finish())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    item: Item,
    code: ItemCode,
    fingerprint: Fingerprint,
    footprint: Footprint,
    carry_group: Option<CarryGroup>,
}

impl Object {
    pub fn new(item: Item) -> Result<Self, ModelError> {
        let code = item.code().unwrap_or(ItemCode::EAR);
        let footprint = tables::footprint(code)?;
        Ok(Self {
            code,
            fingerprint: Fingerprint::of(&item)?,
            footprint,
            carry_group: carry_group(&item),
            item,
        })
    }

    #[must_use]
    pub const fn item(&self) -> &Item {
        &self.item
    }

    #[must_use]
    pub const fn code(&self) -> ItemCode {
        self.code
    }

    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    #[must_use]
    pub const fn footprint(&self) -> Footprint {
        self.footprint
    }

    #[must_use]
    pub const fn carry_group(&self) -> Option<CarryGroup> {
        self.carry_group
    }

    #[must_use]
    pub fn into_item(self) -> Item {
        self.item
    }
}

fn carry_group(item: &Item) -> Option<CarryGroup> {
    let ItemBody::Complete(body) = &item.body else {
        return None;
    };
    let ItemQuality::Unique(file_index) = &body.quality else {
        return None;
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a unique index holds 12 bits"
    )]
    let id = UniqueId::new(file_index.get() as u16);
    tables::carry_group(id)
}
