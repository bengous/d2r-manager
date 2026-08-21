#![allow(
    dead_code,
    reason = "each test binary uses a subset of this shared helper"
)]
#![allow(
    clippy::disallowed_methods,
    reason = "integration tests read the frozen corpus"
)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use domain::container::{CharacterName, StashName};
use format::bits::Bits;
use format::code::ItemCode;
use format::item::{
    BodyLocation, CompactBody, CompactContent, CompleteBody, GridCell, Item, ItemBody, ItemFlags,
    ItemName, ItemPosition, ItemQuality, ItemVersion, SlotMode, Sockets, StorePage, TypeSpecific,
};
use format::save::CharacterSave;
use format::stash::StashSave;
use format::stats::ItemStats;

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

pub(crate) fn read_bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

pub(crate) fn fixture_bytes(relative: &str) -> Vec<u8> {
    read_bytes(&repo_root().join("fixtures").join(relative))
}

pub(crate) fn merge_bytes(name: &str) -> Vec<u8> {
    read_bytes(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/merge-2026-08-20")
            .join(name),
    )
}

pub(crate) fn fixture_names(directory: &str) -> Vec<String> {
    let path = repo_root().join("fixtures").join(directory);
    let mut names: Vec<String> = std::fs::read_dir(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .map(|entry| entry.expect("directory entry").file_name())
        .filter_map(|name| name.into_string().ok())
        .filter(|name| matches!(extension(name), Some("d2s" | "d2i")))
        .collect();
    names.sort();
    names
}

pub(crate) fn extension(name: &str) -> Option<&str> {
    Path::new(name).extension().and_then(OsStr::to_str)
}

pub(crate) fn stash(relative: &str) -> StashSave {
    StashSave::read(&fixture_bytes(relative)).expect("stash save")
}

pub(crate) fn character(relative: &str) -> CharacterSave {
    CharacterSave::read(&fixture_bytes(relative)).expect("character save")
}

pub(crate) fn stash_name(name: &str) -> StashName {
    StashName::new(name).expect("stash name")
}

pub(crate) fn character_name(name: &str) -> CharacterName {
    CharacterName::new(name).expect("character name")
}

fn origin() -> ItemPosition {
    ItemPosition::Slot {
        mode: SlotMode::Stored,
        body: BodyLocation::None,
        cell: GridCell::new(0, 0).expect("cell"),
        page: StorePage::Stash,
    }
}

pub(crate) fn compact(code: &str) -> Item {
    Item {
        flags: ItemFlags::from_bits(ItemFlags::COMPACT_SAVE),
        version: ItemVersion::from_number(101).expect("item version"),
        position: origin(),
        body: ItemBody::Compact(CompactBody {
            content: CompactContent::Code {
                code: ItemCode::from_text(code).expect("item code"),
                gold: None,
            },
            quest_difficulty: None,
            realm: None,
            advanced_stash_stack: Some(Bits::<8>::ZERO),
        }),
    }
}

pub(crate) fn compact_marked(code: &str, mark: u32) -> Item {
    let mut item = compact(code);
    if let ItemBody::Compact(body) = &mut item.body {
        body.realm = Some([mark, 0, 0, 0]);
    }
    item
}

pub(crate) fn unique(code: &str, file_index: u32) -> Item {
    Item {
        flags: ItemFlags::from_bits(ItemFlags::IDENTIFIED),
        version: ItemVersion::from_number(101).expect("item version"),
        position: origin(),
        body: ItemBody::Complete(Box::new(CompleteBody {
            code: ItemCode::from_text(code).expect("item code"),
            seed: 0,
            level: Bits::<7>::new(1).expect("item level"),
            quality: ItemQuality::Unique(Bits::<12>::new(file_index).expect("unique index")),
            variable_gfx: None,
            auto_affix: None,
            runeword: None,
            name: ItemName::Absent,
            realm: None,
            type_specific: TypeSpecific::Absent,
            quantity: None,
            sockets: Sockets {
                filled: Vec::new(),
                capacity: None,
            },
            stats: ItemStats::default(),
            chronicle: None,
            advanced_stash_stack: Some(Bits::<8>::ZERO),
        })),
    }
}

pub(crate) fn place(item: &Item, column: u32, row: u32) -> Item {
    let mut placed = item.clone();
    if let ItemPosition::Slot { cell, .. } = &mut placed.position {
        *cell = GridCell::new(column, row).expect("cell");
    }
    placed
}

pub(crate) fn stacked(item: &Item, count: u32) -> Item {
    let mut piled = item.clone();
    let stack = Some(Bits::<8>::new(count).expect("pile stack"));
    match &mut piled.body {
        ItemBody::Compact(body) => body.advanced_stash_stack = stack,
        ItemBody::Complete(body) => body.advanced_stash_stack = stack,
    }
    piled
}
