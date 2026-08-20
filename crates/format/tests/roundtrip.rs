mod corpus;

use corpus::{fixture_bytes, fixture_names, is_stash};
use format::checksum;
use format::error::FormatError;
use format::save::CharacterSave;
use format::stash::StashSave;

const LINUX: &str = "v105/linux";
const LEGACY: &str = "legacy";

fn rewrite(name: &str, data: &[u8]) -> Result<Vec<u8>, FormatError> {
    if is_stash(name) {
        StashSave::read(data)?.write()
    } else {
        CharacterSave::read(data)?.write()
    }
}

#[test]
fn every_version_105_linux_fixture_survives_a_read_then_write() {
    let names = fixture_names(LINUX);
    assert_eq!(names.len(), 55, "corpus size");

    let mut failures = Vec::new();
    for name in &names {
        let data = fixture_bytes(LINUX, name);
        match rewrite(name, &data) {
            Ok(written) if written == data => {}
            Ok(written) => {
                let at = written
                    .iter()
                    .zip(&data)
                    .position(|(left, right)| left != right);
                failures.push(format!(
                    "{name}: {} bytes written for {} read, first difference at {at:?}",
                    written.len(),
                    data.len()
                ));
            }
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} fixtures failed:\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
}

#[test]
fn the_checksum_of_every_character_fixture_is_reproduced() {
    let mut failures = Vec::new();
    let mut checked = 0usize;
    for name in fixture_names(LINUX) {
        if is_stash(&name) {
            continue;
        }
        checked += 1;
        let data = fixture_bytes(LINUX, &name);
        let stored = checksum::stored(&data).expect("checksum field");
        let computed = checksum::compute(&data);
        if stored != computed {
            failures.push(format!(
                "{name}: stored {stored:#010X}, computed {computed:#010X}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(checked, 53, "character fixtures covered");
}

#[test]
fn every_legacy_fixture_is_refused_with_its_version() {
    let names = fixture_names(LEGACY);
    assert_eq!(names.len(), 3, "legacy corpus size");

    let mut seen = Vec::new();
    for name in &names {
        let data = fixture_bytes(LEGACY, name);
        let error = rewrite(name, &data).expect_err("legacy fixture must be refused");
        match error {
            FormatError::UnsupportedVersion { version } => seen.push((name.clone(), version)),
            other => panic!("{name}: expected an unsupported version error, found {other}"),
        }
    }
    seen.sort();
    assert_eq!(
        seen,
        vec![
            ("BasesTwo.d2s".to_owned(), 99),
            ("JewelsCraft.d2s".to_owned(), 98),
            ("SharedStashHardCoreV2.d2i".to_owned(), 99),
        ]
    );
}
