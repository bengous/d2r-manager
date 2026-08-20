use crate::bits::{BitReader, BitWriter};
use crate::code::ItemCode;
use crate::error::FormatError;

/// FORMAT.md §5.4
const TABLE: [(u8, u32, u32); 37] = [
    (b'0', 0b1111_1011, 8),
    (b' ', 0b10, 2),
    (b'1', 0b111_1100, 7),
    (b'2', 0b00_1100, 6),
    (b'3', 0b110_1101, 7),
    (b'4', 0b1111_1010, 8),
    (b'5', 0b0001_0110, 8),
    (b'6', 0b110_1111, 7),
    (b'7', 0b0_1111, 5),
    (b'8', 0b00_0100, 6),
    (b'9', 0b0_1110, 5),
    (b'a', 0b1_1110, 5),
    (b'b', 0b0101, 4),
    (b'c', 0b0_1000, 5),
    (b'd', 0b11_0001, 6),
    (b'e', 0b11_0000, 6),
    (b'f', 0b01_0011, 6),
    (b'g', 0b1_1010, 5),
    (b'h', 0b0_0011, 5),
    (b'i', 0b111_1110, 7),
    (b'j', 0b0_0010_1110, 9),
    (b'k', 0b01_0010, 6),
    (b'l', 0b1_1101, 5),
    (b'm', 0b0_1101, 5),
    (b'n', 0b00_1101, 6),
    (b'o', 0b111_1111, 7),
    (b'p', 0b1_1001, 5),
    (b'q', 0b1101_1001, 8),
    (b'r', 0b1_1100, 5),
    (b's', 0b0010, 4),
    (b't', 0b0_1100, 5),
    (b'u', 0b0_0001, 5),
    (b'v', 0b110_1110, 7),
    (b'w', 0b0_0000, 5),
    (b'x', 0b0_0111, 5),
    (b'y', 0b000_1010, 7),
    (b'z', 0b1101_1000, 8),
];

const MAX_CODE_LENGTH: u32 = 9;

const fn reverse(code: u32, length: u32) -> u32 {
    let mut source = code;
    let mut reversed = 0;
    let mut index = 0;
    while index < length {
        reversed = reversed << 1 | source & 1;
        source >>= 1;
        index += 1;
    }
    reversed
}

fn symbol(code: u32, length: u32) -> Option<u8> {
    TABLE
        .iter()
        .find(|(_, bits, width)| *width == length && *bits == code)
        .map(|(symbol, _, _)| *symbol)
}

fn encoding(symbol: u8) -> Option<(u32, u32)> {
    TABLE
        .iter()
        .find(|(candidate, _, _)| *candidate == symbol)
        .map(|(_, bits, length)| (reverse(*bits, *length), *length))
}

/// FORMAT.md §5.4
pub fn decode(reader: &mut BitReader<'_>) -> Result<ItemCode, FormatError> {
    let mut bytes = [0u8; 4];
    for slot in &mut bytes {
        let mut code = 0u32;
        let mut length = 0u32;
        loop {
            code = code << 1 | reader.read_bits(1)?;
            length += 1;
            if let Some(found) = symbol(code, length) {
                *slot = found;
                break;
            }
            if length == MAX_CODE_LENGTH {
                return Err(FormatError::HuffmanSymbolUnknown { code, length });
            }
        }
    }
    ItemCode::new(bytes)
}

/// FORMAT.md §5.4
pub fn encode(writer: &mut BitWriter, code: ItemCode) -> Result<(), FormatError> {
    for byte in code.bytes() {
        let (bits, length) = encoding(byte).ok_or(FormatError::ItemCodeOutsideAlphabet { byte })?;
        writer.write_bits(bits, length)?;
    }
    Ok(())
}
