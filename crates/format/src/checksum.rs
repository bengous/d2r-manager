use crate::error::FormatError;

/// FORMAT.md §2.4
pub const FIELD_OFFSET: usize = 0x0C;

/// FORMAT.md §2.4
#[must_use]
pub fn compute(data: &[u8]) -> u32 {
    let mut checksum = 0u32;
    for (index, byte) in data.iter().enumerate() {
        let byte = if (FIELD_OFFSET..FIELD_OFFSET + 4).contains(&index) {
            0
        } else {
            *byte
        };
        checksum = checksum.rotate_left(1).wrapping_add(u32::from(byte));
    }
    checksum
}

/// FORMAT.md §2.4
pub fn stored(data: &[u8]) -> Result<u32, FormatError> {
    data.get(FIELD_OFFSET..FIELD_OFFSET + 4)
        .and_then(|slice| <[u8; 4]>::try_from(slice).ok())
        .map(u32::from_le_bytes)
        .ok_or(FormatError::FileSizeMismatch {
            declared: 0,
            actual: data.len(),
        })
}
