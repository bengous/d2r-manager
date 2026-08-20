use thiserror::Error;

use format::code::ItemCode;
use format::error::FormatError;

use crate::container::{CharacterName, ContainerId, GridId};
use crate::gold::Gold;
use crate::grid::{GridGeometry, GridPosition};
use crate::object::Fingerprint;
use crate::pile::PileUnits;
use crate::tables::{CarryGroup, Footprint};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValueError {
    #[error("gold amount {amount} is above the {cap} a single tab holds")]
    GoldOverCap { amount: u32, cap: u32 },

    #[error("pile count {count} is above the {max} a single pile holds")]
    PileCountOverCap { count: u16, max: u8 },

    #[error("a container name cannot be empty")]
    EmptyName,

    #[error("a fingerprint reads as {expected} hexadecimal digits, not {length}")]
    FingerprintLength { length: usize, expected: usize },

    #[error("a fingerprint holds hexadecimal digits only, not {found:?}")]
    FingerprintDigit { found: char },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModelError {
    #[error("bitstream: {0}")]
    Format(#[from] FormatError),

    #[error("value: {0}")]
    Value(#[from] ValueError),

    #[error("item code {code} has no size in the version 105 tables")]
    UnknownItemSize { code: ItemCode },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VaultError {
    #[error("{container}: {source}")]
    Object {
        container: ContainerId,
        #[source]
        source: ModelError,
    },

    #[error("{container}: gold: {source}")]
    Gold {
        container: ContainerId,
        #[source]
        source: ValueError,
    },

    #[error("{container}: pile entry: {source}")]
    Pile {
        container: ContainerId,
        #[source]
        source: ValueError,
    },

    #[error("{container}: {violation}")]
    Placement {
        container: ContainerId,
        #[source]
        violation: RuleViolation,
    },

    #[error("{container}: an item outside the Stored mode has no place in a container")]
    NotStored { container: ContainerId },

    #[error("{container} is declared twice")]
    ContainerDeclaredTwice { container: ContainerId },

    #[error("stash {stash} declares {tabs} tabs, more than the {max} an index carries")]
    TooManyTabs {
        stash: crate::container::StashName,
        tabs: usize,
        max: u8,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuleViolation {
    #[error("{container} is absent from the vault")]
    UnknownContainer { container: ContainerId },

    #[error("{container} holds no item with the fingerprint {fingerprint}")]
    ObjectAbsent {
        container: ContainerId,
        fingerprint: Fingerprint,
    },

    #[error("{container} holds {held} units of {fingerprint}, fewer than the {asked} asked")]
    NotEnoughUnits {
        container: ContainerId,
        fingerprint: Fingerprint,
        held: u16,
        asked: PileUnits,
    },

    #[error("{container} is a grid: it takes one item per operation, not {asked} units")]
    GridTakesOneUnit { container: GridId, asked: PileUnits },

    #[error(
        "{container}: a {}x{} item at column {} row {} leaves the {}x{} grid",
        footprint.width(),
        footprint.height(),
        cell.column(),
        cell.row(),
        geometry.columns(),
        geometry.rows()
    )]
    OutsideGrid {
        container: GridId,
        cell: GridPosition,
        footprint: Footprint,
        geometry: GridGeometry,
    },

    #[error(
        "{container}: cell column {} row {} is already taken",
        cell.column(),
        cell.row()
    )]
    CellTaken {
        container: GridId,
        cell: GridPosition,
    },

    #[error(
        "{container} has no free {}x{} space left",
        footprint.width(),
        footprint.height()
    )]
    GridFull {
        container: GridId,
        footprint: Footprint,
    },

    #[error("{character} would carry {count} items of the exclusive group {}, more than one", group.get())]
    CarryLimitExceeded {
        character: CharacterName,
        group: CarryGroup,
        count: u16,
    },

    #[error("{container} is a grid, not a pile")]
    NotAPile { container: ContainerId },

    #[error("{container} is a pile, not a grid")]
    NotAGrid { container: ContainerId },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("operation {index}: {violation}")]
pub struct PlanError {
    pub index: usize,
    #[source]
    pub violation: RuleViolation,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ApplyError {
    #[error("{container} holds {observed} units of {fingerprint}, the plan expected {expected}")]
    UnitsDiffer {
        container: ContainerId,
        fingerprint: Fingerprint,
        expected: u32,
        observed: u32,
    },

    #[error("{container} is absent from the vault read back after writing")]
    ContainerMissing { container: ContainerId },

    #[error("{container} is absent from the outcome the plan expected")]
    ContainerUnexpected { container: ContainerId },

    #[error("{container} holds {observed} gold, the plan expected {expected}")]
    GoldDiffers {
        container: ContainerId,
        expected: Gold,
        observed: Gold,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PileError {
    #[error("pile: {0}")]
    Value(#[from] ValueError),

    #[error("pile holds {held} units of {fingerprint}, fewer than the {asked} asked")]
    NotEnoughUnits {
        fingerprint: Fingerprint,
        held: u16,
        asked: PileUnits,
    },
}
