use format::bits::{BitError, BitReader, BitWriter, Bits};
use proptest::prelude::*;

fn field() -> impl Strategy<Value = (u32, u32)> {
    (1u32..=32, any::<u32>()).prop_map(|(width, raw)| {
        let mask = if width == 32 {
            u32::MAX
        } else {
            (1u32 << width) - 1
        };
        (raw & mask, width)
    })
}

fn signed_field() -> impl Strategy<Value = (i32, u32)> {
    (1u32..=32, any::<i32>()).prop_map(|(width, raw)| {
        if width == 32 {
            return (raw, width);
        }
        let span = 1i64 << (width - 1);
        let low = -span;
        let high = span - 1;
        let wrapped = i64::from(raw).rem_euclid(span * 2) + low;
        let value = i32::try_from(wrapped.clamp(low, high)).unwrap_or(0);
        (value, width)
    })
}

proptest! {
    #[test]
    fn unsigned_fields_survive_a_write_then_read(fields in prop::collection::vec(field(), 0..64)) {
        let mut writer = BitWriter::new();
        for &(value, width) in &fields {
            writer.write_bits(value, width).unwrap();
        }
        let bytes = writer.finish();

        let mut reader = BitReader::new(&bytes);
        for &(value, width) in &fields {
            prop_assert_eq!(reader.read_bits(width).unwrap(), value);
        }
    }

    #[test]
    fn signed_fields_survive_a_write_then_read(fields in prop::collection::vec(signed_field(), 0..64)) {
        let mut writer = BitWriter::new();
        for &(value, width) in &fields {
            writer.write_signed_bits(value, width).unwrap();
        }
        let bytes = writer.finish();

        let mut reader = BitReader::new(&bytes);
        for &(value, width) in &fields {
            prop_assert_eq!(reader.read_signed_bits(width).unwrap(), value);
        }
    }

    #[test]
    fn a_write_uses_exactly_the_requested_bits(fields in prop::collection::vec(field(), 0..64)) {
        let mut writer = BitWriter::new();
        let mut total = 0usize;
        for &(value, width) in &fields {
            writer.write_bits(value, width).unwrap();
            total += width as usize;
        }
        prop_assert_eq!(writer.bit_position(), total);
        prop_assert_eq!(writer.byte_len(), total.div_ceil(8));
    }

    #[test]
    fn a_short_read_reports_the_gap_and_leaves_the_position(data in prop::collection::vec(any::<u8>(), 0..32), width in 1u32..=32) {
        let mut reader = BitReader::new(&data);
        let available = reader.bits_left();
        let skip = available.min(3);
        reader.seek(skip);

        let left = reader.bits_left();
        let result = reader.read_bits(width);
        if left < width as usize {
            prop_assert_eq!(result, Err(BitError::EndOfData { position: skip, needed: width, available: left }));
            prop_assert_eq!(reader.bit_position(), skip);
        } else {
            prop_assert!(result.is_ok());
            prop_assert_eq!(reader.bit_position(), skip + width as usize);
        }
    }

    #[test]
    fn align_moves_to_the_next_byte_boundary(data in prop::collection::vec(any::<u8>(), 1..32), start in 0usize..64) {
        let mut reader = BitReader::new(&data);
        reader.seek(start);
        reader.align();
        let aligned = reader.bit_position();
        prop_assert!(aligned.is_multiple_of(8));
        prop_assert!(aligned >= start);
        prop_assert!(aligned - start < 8);
        reader.align();
        prop_assert_eq!(reader.bit_position(), aligned);
    }

    #[test]
    fn reads_are_little_endian_within_each_byte(data in prop::collection::vec(any::<u8>(), 4..32)) {
        let mut reader = BitReader::new(&data);
        let mut rebuilt = 0u32;
        for index in 0..32 {
            rebuilt |= reader.read_bits(1).unwrap() << index;
        }
        let expected = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        prop_assert_eq!(rebuilt, expected);
    }

    #[test]
    fn byte_reads_need_alignment(data in prop::collection::vec(any::<u8>(), 8..32), start in 0usize..16) {
        let mut reader = BitReader::new(&data);
        reader.seek(start);
        let result = reader.read_bytes(1);
        if start.is_multiple_of(8) {
            prop_assert_eq!(result.unwrap(), &data[start / 8..=start / 8]);
        } else {
            prop_assert_eq!(result, Err(BitError::Unaligned { position: start }));
        }
    }
}

#[test]
fn a_typed_field_rejects_a_value_wider_than_its_width() {
    assert_eq!(Bits::<3>::MAX, 7);
    assert!(Bits::<3>::new(7).is_some());
    assert!(Bits::<3>::new(8).is_none());
    assert_eq!(Bits::<32>::MAX, u32::MAX);
}
