use std::fmt;

use crate::bits::{BitReader, BitWriter};
use crate::error::FormatError;

/// FORMAT.md §2.2
pub const MAX_ITEM_STRING: usize = 63;

/// FORMAT.md §2.2
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveString(Vec<u8>);

impl SaveString {
    pub fn new(bytes: Vec<u8>) -> Result<Self, FormatError> {
        if bytes.len() > MAX_ITEM_STRING {
            return Err(FormatError::StringTooLong {
                length: bytes.len(),
                max: MAX_ITEM_STRING,
            });
        }
        if let Some(byte) = bytes.iter().copied().find(|byte| *byte == 0) {
            return Err(FormatError::StringByteUnwritable { byte });
        }
        Ok(Self(bytes))
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    /// FORMAT.md §2.2
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        let mut bytes = Vec::new();
        loop {
            let byte = reader.read_u8()?;
            if byte == 0 {
                return Self::new(bytes);
            }
            if bytes.len() == MAX_ITEM_STRING {
                return Err(FormatError::StringTooLong {
                    length: bytes.len() + 1,
                    max: MAX_ITEM_STRING,
                });
            }
            bytes.push(byte);
        }
    }

    /// FORMAT.md §2.2
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        for byte in &self.0 {
            writer.write_u8(*byte)?;
        }
        writer.write_u8(0)?;
        Ok(())
    }
}

impl fmt::Display for SaveString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&String::from_utf8_lossy(&self.0))
    }
}

/// FORMAT.md §2.2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedString<const N: usize>([u8; N]);

impl<const N: usize> FixedString<N> {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(&self) -> &[u8; N] {
        &self.0
    }

    #[must_use]
    pub fn text(&self) -> &[u8] {
        let end = self.0.iter().position(|byte| *byte == 0).unwrap_or(N);
        &self.0[..end]
    }

    /// FORMAT.md §2.2
    pub fn read(reader: &mut BitReader<'_>) -> Result<Self, FormatError> {
        Ok(Self(reader.read_array::<N>()?))
    }

    /// FORMAT.md §2.2
    pub fn write(&self, writer: &mut BitWriter) -> Result<(), FormatError> {
        writer.write_bytes(&self.0)?;
        Ok(())
    }
}

impl<const N: usize> Default for FixedString<N> {
    fn default() -> Self {
        Self([0; N])
    }
}

impl<const N: usize> fmt::Display for FixedString<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&String::from_utf8_lossy(self.text()))
    }
}
