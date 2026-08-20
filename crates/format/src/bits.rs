use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BitError {
    #[error("end of data at bit {position}: {needed} bit(s) needed, {available} left")]
    EndOfData {
        position: usize,
        needed: u32,
        available: usize,
    },
    #[error("byte access at bit {position} is not byte aligned")]
    Unaligned { position: usize },
    #[error("value {value} does not fit in {width} bit(s)")]
    ValueTooWide { value: u32, width: u32 },
    #[error("width {width} is out of the 1..=32 range")]
    WidthOutOfRange { width: u32 },
}

/// FORMAT.md §2.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bits<const N: u32>(u32);

impl<const N: u32> Bits<N> {
    const WIDTH_IS_VALID: () = assert!(N != 0 && N <= 32, "Bits<N> requires 1 <= N <= 32");

    pub const MAX: u32 = {
        let () = Self::WIDTH_IS_VALID;
        if N == 32 { u32::MAX } else { (1u32 << N) - 1 }
    };

    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl<const N: u32> TryFrom<u32> for Bits<N> {
    type Error = BitError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(BitError::ValueTooWide { value, width: N })
    }
}

/// FORMAT.md §2.1
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> BitReader<'a> {
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    #[must_use]
    pub const fn bit_position(&self) -> usize {
        self.position
    }

    #[must_use]
    pub const fn bits_left(&self) -> usize {
        (self.data.len() * 8).saturating_sub(self.position)
    }

    pub const fn seek(&mut self, bit_position: usize) {
        self.position = bit_position;
    }

    pub fn byte_position(&self) -> Result<usize, BitError> {
        if self.position.is_multiple_of(8) {
            Ok(self.position / 8)
        } else {
            Err(BitError::Unaligned {
                position: self.position,
            })
        }
    }

    pub const fn align(&mut self) {
        let remainder = self.position % 8;
        if remainder != 0 {
            self.position += 8 - remainder;
        }
    }

    pub fn read_bits(&mut self, width: u32) -> Result<u32, BitError> {
        if width == 0 {
            return Ok(0);
        }
        if width > 32 {
            return Err(BitError::WidthOutOfRange { width });
        }
        if self.bits_left() < width as usize {
            return Err(BitError::EndOfData {
                position: self.position,
                needed: width,
                available: self.bits_left(),
            });
        }

        let mut value = 0u32;
        let mut done = 0u32;
        while done < width {
            let byte = self.data[self.position / 8];
            #[expect(clippy::cast_possible_truncation, reason = "position % 8 is 0..=7")]
            let offset = (self.position % 8) as u32;
            let take = (8 - offset).min(width - done);
            let mask = if take == 32 {
                u32::MAX
            } else {
                (1u32 << take) - 1
            };
            value |= (u32::from(byte) >> offset & mask) << done;
            self.position += take as usize;
            done += take;
        }
        Ok(value)
    }

    pub fn read<const N: u32>(&mut self) -> Result<Bits<N>, BitError> {
        self.read_bits(N).map(Bits)
    }

    pub fn read_signed_bits(&mut self, width: u32) -> Result<i32, BitError> {
        let raw = self.read_bits(width)?;
        if width == 0 || width >= 32 {
            #[expect(
                clippy::cast_possible_wrap,
                reason = "two's complement reinterpretation"
            )]
            return Ok(raw as i32);
        }
        let sign = 1u32 << (width - 1);
        let extended = if raw & sign == 0 {
            raw
        } else {
            raw | !(sign - 1)
        };
        #[expect(
            clippy::cast_possible_wrap,
            reason = "two's complement reinterpretation"
        )]
        Ok(extended as i32)
    }

    pub fn read_bool(&mut self) -> Result<bool, BitError> {
        Ok(self.read_bits(1)? != 0)
    }

    pub fn read_u8(&mut self) -> Result<u8, BitError> {
        #[expect(clippy::cast_possible_truncation, reason = "8 bits requested")]
        Ok(self.read_bits(8)? as u8)
    }

    pub fn read_u16(&mut self) -> Result<u16, BitError> {
        #[expect(clippy::cast_possible_truncation, reason = "16 bits requested")]
        Ok(self.read_bits(16)? as u16)
    }

    pub fn read_u32(&mut self) -> Result<u32, BitError> {
        self.read_bits(32)
    }

    pub fn read_u64(&mut self) -> Result<u64, BitError> {
        let low = u64::from(self.read_bits(32)?);
        let high = u64::from(self.read_bits(32)?);
        Ok(high << 32 | low)
    }

    pub fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], BitError> {
        let start = self.byte_position()?;
        let end = start.checked_add(count).ok_or(BitError::EndOfData {
            position: self.position,
            needed: u32::MAX,
            available: self.bits_left(),
        })?;
        if end > self.data.len() {
            return Err(BitError::EndOfData {
                position: self.position,
                needed: u32::try_from(count * 8).unwrap_or(u32::MAX),
                available: self.bits_left(),
            });
        }
        self.position = end * 8;
        Ok(&self.data[start..end])
    }

    pub fn read_u32_array<const N: usize>(&mut self) -> Result<[u32; N], BitError> {
        let mut out = [0u32; N];
        for slot in &mut out {
            *slot = self.read_u32()?;
        }
        Ok(out)
    }

    pub fn read_array<const N: usize>(&mut self) -> Result<[u8; N], BitError> {
        let bytes = self.read_bytes(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }
}

/// FORMAT.md §2.1
#[derive(Debug, Default, Clone)]
pub struct BitWriter {
    data: Vec<u8>,
    position: usize,
}

impl BitWriter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            data: Vec::new(),
            position: 0,
        }
    }

    #[must_use]
    pub const fn bit_position(&self) -> usize {
        self.position
    }

    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.data
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn align(&mut self) {
        let remainder = self.position % 8;
        if remainder != 0 {
            self.position += 8 - remainder;
        }
    }

    pub fn write_bits(&mut self, value: u32, width: u32) -> Result<(), BitError> {
        if width == 0 {
            return Ok(());
        }
        if width > 32 {
            return Err(BitError::WidthOutOfRange { width });
        }
        let max = if width == 32 {
            u32::MAX
        } else {
            (1u32 << width) - 1
        };
        if value > max {
            return Err(BitError::ValueTooWide { value, width });
        }

        let mut done = 0u32;
        while done < width {
            let index = self.position / 8;
            if index >= self.data.len() {
                self.data.resize(index + 1, 0);
            }
            #[expect(clippy::cast_possible_truncation, reason = "position % 8 is 0..=7")]
            let offset = (self.position % 8) as u32;
            let take = (8 - offset).min(width - done);
            let mask = if take == 32 {
                u32::MAX
            } else {
                (1u32 << take) - 1
            };
            #[expect(clippy::cast_possible_truncation, reason = "take <= 8 bits")]
            let chunk = ((value >> done & mask) << offset) as u8;
            self.data[index] |= chunk;
            self.position += take as usize;
            done += take;
        }
        Ok(())
    }

    pub fn write<const N: u32>(&mut self, value: Bits<N>) -> Result<(), BitError> {
        self.write_bits(value.get(), N)
    }

    pub fn write_signed_bits(&mut self, value: i32, width: u32) -> Result<(), BitError> {
        #[expect(clippy::cast_sign_loss, reason = "two's complement reinterpretation")]
        let raw = value as u32;
        let masked = if width >= 32 {
            raw
        } else {
            raw & ((1u32 << width) - 1)
        };
        self.write_bits(masked, width)
    }

    pub fn write_bool(&mut self, value: bool) -> Result<(), BitError> {
        self.write_bits(u32::from(value), 1)
    }

    pub fn write_u8(&mut self, value: u8) -> Result<(), BitError> {
        self.write_bits(u32::from(value), 8)
    }

    pub fn write_u16(&mut self, value: u16) -> Result<(), BitError> {
        self.write_bits(u32::from(value), 16)
    }

    pub fn write_u32(&mut self, value: u32) -> Result<(), BitError> {
        self.write_bits(value, 32)
    }

    pub fn write_u64(&mut self, value: u64) -> Result<(), BitError> {
        #[expect(clippy::cast_possible_truncation, reason = "low half taken on purpose")]
        let low = value as u32;
        let high = (value >> 32) as u32;
        self.write_bits(low, 32)?;
        self.write_bits(high, 32)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), BitError> {
        if !self.position.is_multiple_of(8) {
            return Err(BitError::Unaligned {
                position: self.position,
            });
        }
        let start = self.position / 8;
        if self.data.len() < start + bytes.len() {
            self.data.resize(start + bytes.len(), 0);
        }
        self.data[start..start + bytes.len()].copy_from_slice(bytes);
        self.position = (start + bytes.len()) * 8;
        Ok(())
    }

    pub fn patch_u16(&mut self, byte_offset: usize, value: u16) {
        self.data[byte_offset..byte_offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    pub fn patch_u32(&mut self, byte_offset: usize, value: u32) {
        self.data[byte_offset..byte_offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
