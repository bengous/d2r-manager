use std::num::NonZeroU8;

use format::code::ItemCode;

use crate::error::ModelError;

mod v105;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Footprint {
    width: NonZeroU8,
    height: NonZeroU8,
}

impl Footprint {
    pub const SPAN: u8 = 16;

    pub(crate) const fn new(width: u8, height: u8) -> Self {
        assert!(
            width <= Self::SPAN && height <= Self::SPAN,
            "a footprint stays inside the 16 by 16 cells a position addresses"
        );
        Self {
            width: NonZeroU8::new(width).expect("table footprint width is not zero"),
            height: NonZeroU8::new(height).expect("table footprint height is not zero"),
        }
    }

    #[must_use]
    pub const fn width(self) -> u8 {
        self.width.get()
    }

    #[must_use]
    pub const fn height(self) -> u8 {
        self.height.get()
    }

    #[must_use]
    pub const fn cells(self) -> u16 {
        self.width.get() as u16 * self.height.get() as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UniqueId(u16);

impl UniqueId {
    #[must_use]
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CarryGroup(u16);

impl CarryGroup {
    #[must_use]
    pub const fn new(group: u16) -> Self {
        Self(group)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

pub fn footprint(code: ItemCode) -> Result<Footprint, ModelError> {
    v105::FOOTPRINTS
        .binary_search_by_key(&code.as_u32(), |(key, _)| *key)
        .map(|index| v105::FOOTPRINTS[index].1)
        .map_err(|_| ModelError::UnknownItemSize { code })
}

#[must_use]
pub fn carry_group(unique: UniqueId) -> Option<CarryGroup> {
    v105::CARRY_GROUPS
        .binary_search_by_key(&unique.get(), |(key, _)| *key)
        .ok()
        .map(|index| v105::CARRY_GROUPS[index].1)
}
