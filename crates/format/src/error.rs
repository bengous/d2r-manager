use thiserror::Error;

use crate::bits::BitError;
use crate::code::ItemCode;

pub const SUPPORTED_VERSION: u32 = 105;

/// FORMAT.md §1.1
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FormatError {
    #[error("bitstream: {0}")]
    Bits(#[from] BitError),

    #[error("save version {version} is not supported, only {SUPPORTED_VERSION} is")]
    UnsupportedVersion { version: u32 },

    #[error("{section}: expected magic 0x{expected:08X}, found 0x{found:08X} at byte {at}")]
    BadMagic {
        section: &'static str,
        expected: u32,
        found: u32,
        at: usize,
    },

    #[error("{section}: expected size {expected}, found {found}")]
    BadSectionSize {
        section: &'static str,
        expected: usize,
        found: usize,
    },

    #[error("item code byte 0x{byte:02X} is outside the Huffman alphabet")]
    ItemCodeOutsideAlphabet { byte: u8 },

    #[error("no Huffman symbol for the {length}-bit code 0x{code:X}")]
    HuffmanSymbolUnknown { code: u32, length: u32 },

    #[error("item code {code} is absent from the version {SUPPORTED_VERSION} tables")]
    UnknownItemCode { code: ItemCode },

    #[error("stat id {id} is absent from the version {SUPPORTED_VERSION} tables")]
    UnknownStatId { id: u16 },

    #[error("stat id {id} is the list terminator and cannot name a stat")]
    StatIdIsTerminator { id: u16 },

    #[error("stat {id} value {value} does not fit in its {bits}-bit field")]
    StatValueOutOfRange { id: u16, value: i64, bits: u32 },

    #[error("flag {flag} and its payload disagree")]
    FlagPayloadMismatch { flag: &'static str },

    #[error("character class {class} is not one of the eight known classes")]
    UnknownCharacterClass { class: u8 },

    #[error("game version {version} is not one of the four known values")]
    UnknownGameVersion { version: u8 },

    #[error("item version {version} is not representable in the 3-bit field")]
    UnknownItemVersion { version: u16 },

    #[error("item mode {mode} is not one of the seven known modes")]
    UnknownItemMode { mode: u32 },

    #[error("body location {location} is not one of the thirteen known slots")]
    UnknownBodyLocation { location: u32 },

    #[error("item quality {quality} is not one of the nine known qualities")]
    UnknownItemQuality { quality: u32 },

    #[error("grid cell column {column} row {row} does not fit in 4 bits")]
    GridCellOutOfRange { column: u32, row: u32 },

    #[error("string field holds {length} bytes, more than the {max} the game reads")]
    StringTooLong { length: usize, max: usize },

    #[error("string field holds byte 0x{byte:02X}, which the game cannot store")]
    StringByteUnwritable { byte: u8 },

    #[error("the LowQuality flag marks a gamble item, whose record the game truncates")]
    GambleItem,

    #[error("stash tab {tab} declares {declared} bytes but its header alone needs {minimum}")]
    StashTabTooShort {
        tab: usize,
        declared: usize,
        minimum: usize,
    },

    #[error("stash tab {tab} body overruns its declared {declared} bytes by {overrun}")]
    StashTabOverrun {
        tab: usize,
        declared: usize,
        overrun: usize,
    },

    #[error("stash tab type {kind} is not one of Normal, AdvancedStash or Chronicle")]
    UnknownStashTabType { kind: u8 },

    #[error("stash tab declares type {declared} but carries a type {body} body")]
    StashTabKindMismatch { declared: u8, body: u8 },

    #[error("the file holds {actual} bytes, fewer than the {needed} of a header")]
    TruncatedHeader { needed: usize, actual: usize },

    #[error("the written file holds {bytes} bytes, more than a 32-bit size field carries")]
    FileTooLarge { bytes: usize },
}
