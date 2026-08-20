mod corpus;

use corpus::fixture_bytes;
use format::bits::{BitError, BitWriter};
use format::error::FormatError;
use format::stash::{ChronicleSection, StashSave, TabBody};
use format::stats::{ItemStat, ItemStats, StatId};

#[test]
fn patching_past_the_written_bytes_reports_instead_of_panicking() {
    let mut writer = BitWriter::new();
    writer.write_u32(0).expect("four bytes");

    assert!(writer.patch(0, &[1, 2, 3, 4]).is_ok(), "in range");
    assert!(matches!(
        writer.patch(1, &[1, 2, 3, 4]),
        Err(BitError::EndOfData { .. })
    ));
    assert!(matches!(
        writer.patch(1000, &[1, 2]),
        Err(BitError::EndOfData { .. })
    ));
    assert!(matches!(
        writer.patch(usize::MAX, &[1, 2]),
        Err(BitError::EndOfData { .. })
    ));
}

#[test]
fn a_stat_value_that_overflows_its_field_reports_instead_of_panicking() {
    let stats = ItemStats(vec![ItemStat {
        id: StatId::ARMOR_CLASS,
        param: 0,
        value: i64::MAX,
        paired: Vec::new(),
    }]);

    let mut writer = BitWriter::new();
    assert!(matches!(
        stats.write(&mut writer),
        Err(FormatError::StatValueOutOfRange { .. })
    ));
}

#[test]
fn a_stash_tab_body_that_contradicts_its_type_byte_is_refused() {
    let data = fixture_bytes("v105/linux", "ModernSharedStashSoftCoreV2.d2i");
    let mut save = StashSave::read(&data).expect("stash");

    assert_eq!(save.tabs[0].kind_byte(), 0, "the first tab declares Normal");
    save.tabs[0].body = TabBody::Chronicle(ChronicleSection {
        version: 1,
        reserved: [0; 8],
        sets: Vec::new(),
        uniques: Vec::new(),
        runewords: Vec::new(),
    });

    assert!(matches!(
        save.write(),
        Err(FormatError::StashTabKindMismatch {
            declared: 0,
            body: 2
        })
    ));
}
