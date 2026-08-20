use std::fmt;

use crate::error::FormatError;

/// FORMAT.md §5.4
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemCode([u8; 4]);

impl ItemCode {
    pub const EAR: Self = Self(*b"ear ");

    #[must_use]
    pub const fn is_writable(byte: u8) -> bool {
        byte == b' ' || byte.is_ascii_digit() || byte.is_ascii_lowercase()
    }

    pub const fn new(bytes: [u8; 4]) -> Result<Self, FormatError> {
        let mut index = 0;
        while index < 4 {
            if !Self::is_writable(bytes[index]) {
                return Err(FormatError::ItemCodeOutsideAlphabet { byte: bytes[index] });
            }
            index += 1;
        }
        Ok(Self(bytes))
    }

    pub fn from_text(text: &str) -> Result<Self, FormatError> {
        if text.len() > 4 {
            return Err(FormatError::StringTooLong {
                length: text.len(),
                max: 4,
            });
        }
        let mut bytes = [b' '; 4];
        for (slot, byte) in bytes.iter_mut().zip(text.bytes()) {
            *slot = byte;
        }
        Self::new(bytes)
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; 4] {
        self.0
    }

    #[must_use]
    pub const fn as_u32(self) -> u32 {
        u32::from_le_bytes(self.0)
    }
}

impl fmt::Display for ItemCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let trimmed = match self.0.iter().rposition(|byte| *byte != b' ') {
            Some(last) => &self.0[..=last],
            None => &[][..],
        };
        for byte in trimmed {
            write!(formatter, "{}", char::from(*byte))?;
        }
        Ok(())
    }
}
