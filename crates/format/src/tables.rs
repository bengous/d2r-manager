use crate::code::ItemCode;
use crate::error::FormatError;

mod v105;

/// FORMAT.md §6.1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatInfo {
    pub csv_bits: u32,
    pub csv_param: u32,
    pub csv_signed: bool,
    pub save_bits: u32,
    pub save_param_bits: u32,
    pub save_add: i32,
    pub val_shift: u32,
}

impl StatInfo {
    #[must_use]
    pub(crate) const fn new(
        csv_bits: u32,
        csv_param: u32,
        csv_signed: bool,
        save_bits: u32,
        save_param_bits: u32,
        save_add: i32,
        val_shift: u32,
    ) -> Self {
        Self {
            csv_bits,
            csv_param,
            csv_signed,
            save_bits,
            save_param_bits,
            save_add,
            val_shift,
        }
    }
}

/// FORMAT.md §6.2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemInfo(u16);

impl ItemInfo {
    pub const COMPACT_SAVE: u16 = 1 << 0;
    pub const VARIABLE_GFX: u16 = 1 << 1;
    pub const ARMOR: u16 = 1 << 2;
    pub const WEAPON: u16 = 1 << 3;
    pub const GOLD: u16 = 1 << 4;
    pub const CHARM: u16 = 1 << 5;
    pub const BODY_PART: u16 = 1 << 6;
    pub const PLAYER_BODY_PART: u16 = 1 << 7;
    pub const SCROLL_OR_BOOK: u16 = 1 << 8;
    pub const QUEST: u16 = 1 << 9;
    pub const QUEST_DIFF_CHECK: u16 = 1 << 10;

    #[must_use]
    pub(crate) const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    #[must_use]
    const fn has(self, flag: u16) -> bool {
        self.0 & flag != 0
    }

    #[must_use]
    pub const fn compact_save(self) -> bool {
        self.has(Self::COMPACT_SAVE)
    }

    #[must_use]
    pub const fn has_variable_gfx(self) -> bool {
        self.has(Self::VARIABLE_GFX)
    }

    #[must_use]
    pub const fn is_armor(self) -> bool {
        self.has(Self::ARMOR)
    }

    #[must_use]
    pub const fn is_weapon(self) -> bool {
        self.has(Self::WEAPON)
    }

    #[must_use]
    pub const fn is_gold(self) -> bool {
        self.has(Self::GOLD)
    }

    #[must_use]
    pub const fn is_charm(self) -> bool {
        self.has(Self::CHARM)
    }

    #[must_use]
    pub const fn is_body_part(self) -> bool {
        self.has(Self::BODY_PART)
    }

    #[must_use]
    pub const fn is_player_body_part(self) -> bool {
        self.has(Self::PLAYER_BODY_PART)
    }

    #[must_use]
    pub const fn is_scroll_or_book(self) -> bool {
        self.has(Self::SCROLL_OR_BOOK)
    }

    #[must_use]
    pub const fn is_quest(self) -> bool {
        self.has(Self::QUEST)
    }

    #[must_use]
    pub const fn quest_diff_check(self) -> bool {
        self.has(Self::QUEST_DIFF_CHECK)
    }
}

/// FORMAT.md §6.1
pub fn stat_info(id: u16) -> Result<StatInfo, FormatError> {
    v105::STATS
        .get(usize::from(id))
        .copied()
        .ok_or(FormatError::UnknownStatId { id })
}

/// FORMAT.md §6.2
pub fn item_info(code: ItemCode) -> Result<ItemInfo, FormatError> {
    v105::ITEMS
        .binary_search_by_key(&code.as_u32(), |(key, _)| *key)
        .map(|index| v105::ITEMS[index].1)
        .map_err(|_| FormatError::UnknownItemCode { code })
}
