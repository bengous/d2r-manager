use std::collections::BTreeSet;
use std::fmt;

use thiserror::Error;

use domain::census::Census;
use domain::container::ContainerId;
use domain::grid::GridPosition;
use domain::object::{Fingerprint, Object};
use domain::pile::PileCount;
use domain::vault::Vault;
use format::code::ItemCode;
use format::item::{ItemBody, ItemQuality};

use crate::selector::ContainerSelector;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NeedleError {
    #[error("a fingerprint prefix holds hexadecimal digits only, not {found:?}")]
    Prefix { found: String },

    #[error("{found:?} names no quality: {}", Quality::names().join(", "))]
    Quality { found: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Quality {
    Simple,
    Inferior,
    Normal,
    Superior,
    Magic,
    Set,
    Rare,
    Unique,
    Craft,
    Tempered,
}

impl Quality {
    pub const ALL: [Self; 10] = [
        Self::Simple,
        Self::Inferior,
        Self::Normal,
        Self::Superior,
        Self::Magic,
        Self::Set,
        Self::Rare,
        Self::Unique,
        Self::Craft,
        Self::Tempered,
    ];

    #[must_use]
    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|quality| quality.as_str()).collect()
    }

    #[must_use]
    pub fn of(object: &Object) -> Self {
        let ItemBody::Complete(body) = &object.item().body else {
            return Self::Simple;
        };
        match body.quality {
            ItemQuality::Inferior(_) => Self::Inferior,
            ItemQuality::Normal(_) => Self::Normal,
            ItemQuality::Superior(_) => Self::Superior,
            ItemQuality::Magic { .. } => Self::Magic,
            ItemQuality::Set { .. } => Self::Set,
            ItemQuality::Rare(_) => Self::Rare,
            ItemQuality::Unique(_) => Self::Unique,
            ItemQuality::Craft(_) => Self::Craft,
            ItemQuality::Tempered { .. } => Self::Tempered,
        }
    }

    pub fn parse(text: &str) -> Result<Self, NeedleError> {
        Self::ALL
            .into_iter()
            .find(|quality| quality.as_str() == text)
            .ok_or_else(|| NeedleError::Quality {
                found: text.to_owned(),
            })
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Inferior => "inferior",
            Self::Normal => "normal",
            Self::Superior => "superior",
            Self::Magic => "magic",
            Self::Set => "set",
            Self::Rare => "rare",
            Self::Unique => "unique",
            Self::Craft => "craft",
            Self::Tempered => "tempered",
        }
    }
}

impl fmt::Display for Quality {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FingerprintPrefix(Box<str>);

impl FingerprintPrefix {
    pub fn new(text: &str) -> Result<Self, NeedleError> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(NeedleError::Prefix {
                found: text.to_owned(),
            });
        }
        Ok(Self(text.to_ascii_lowercase().into()))
    }

    #[must_use]
    pub fn covers(&self, fingerprint: Fingerprint) -> bool {
        fingerprint.to_string().starts_with(&*self.0)
    }
}

impl fmt::Display for FingerprintPrefix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Cell(GridPosition),
    Units(PileCount),
}

impl fmt::Display for Placement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cell(cell) => write!(formatter, "{},{}", cell.column().get(), cell.row().get()),
            Self::Units(count) => write!(formatter, "x{count}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub container: ContainerSelector,
    pub placement: Placement,
    pub code: Option<ItemCode>,
    pub quality: Quality,
    pub fingerprint: Fingerprint,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Needle {
    pub container: Option<ContainerSelector>,
    pub code: Option<ItemCode>,
    pub quality: Option<Quality>,
    pub fingerprint: Option<FingerprintPrefix>,
}

impl Needle {
    #[must_use]
    pub fn matches(&self, row: &Row) -> bool {
        self.container
            .as_ref()
            .is_none_or(|container| *container == row.container)
            && self.code.is_none_or(|code| Some(code) == row.code)
            && self.quality.is_none_or(|quality| quality == row.quality)
            && self
                .fingerprint
                .as_ref()
                .is_none_or(|prefix| prefix.covers(row.fingerprint))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub container: ContainerSelector,
    pub fingerprint: Fingerprint,
    pub before: u32,
    pub after: u32,
}

#[must_use]
pub fn list(vault: &Vault, needle: &Needle) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for (id, grid) in vault.grids() {
        let container = ContainerSelector::of(&ContainerId::Grid(id.clone()));
        for slot in grid.slots() {
            rows.push(row(
                container.clone(),
                Placement::Cell(slot.origin()),
                slot.object(),
            ));
        }
    }
    for (id, pile) in vault.piles() {
        let container = ContainerSelector::of(&ContainerId::Pile(id.clone()));
        for entry in pile.entries() {
            rows.push(row(
                container.clone(),
                Placement::Units(entry.count()),
                entry.object(),
            ));
        }
    }
    rows.retain(|row| needle.matches(row));
    rows.sort_by(|left, right| sort_key(left).cmp(&sort_key(right)));
    rows
}

#[must_use]
pub fn compare(before: &Vault, after: &Vault) -> Vec<Difference> {
    let held = Census::of(before);
    let wanted = Census::of(after);
    let containers: BTreeSet<&ContainerId> = held
        .containers()
        .chain(wanted.containers())
        .map(|(id, _)| id)
        .collect();
    let mut differences = Vec::new();
    for container in containers {
        let empty = domain::census::Units::new();
        let left = held.units(container).unwrap_or(&empty);
        let right = wanted.units(container).unwrap_or(&empty);
        let prints: BTreeSet<Fingerprint> = left.keys().chain(right.keys()).copied().collect();
        for fingerprint in prints {
            let before = left.get(&fingerprint).copied().unwrap_or_default();
            let after = right.get(&fingerprint).copied().unwrap_or_default();
            if before != after {
                differences.push(Difference {
                    container: ContainerSelector::of(container),
                    fingerprint,
                    before,
                    after,
                });
            }
        }
    }
    differences
}

fn row(container: ContainerSelector, placement: Placement, object: &Object) -> Row {
    Row {
        container,
        placement,
        code: object.item().code(),
        quality: Quality::of(object),
        fingerprint: object.fingerprint(),
    }
}

fn sort_key(row: &Row) -> (&ContainerSelector, u8, u8, u8, Fingerprint) {
    let (row_index, column, units) = match row.placement {
        Placement::Cell(cell) => (cell.row().get(), cell.column().get(), 0),
        Placement::Units(count) => (0, 0, count.get()),
    };
    (&row.container, row_index, column, units, row.fingerprint)
}
