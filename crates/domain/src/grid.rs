use std::fmt;
use std::num::NonZeroU8;

use format::item::GridCell;

use crate::container::GridId;
use crate::error::{ModelError, RuleViolation};
use crate::object::{Fingerprint, Object};
use crate::tables::Footprint;

const ADDRESS_ROWS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Column(u8);

impl Column {
    #[must_use]
    pub const fn new(index: u8) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl fmt::Display for Column {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Row(u8);

impl Row {
    #[must_use]
    pub const fn new(index: u8) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl fmt::Display for Row {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridGeometry {
    columns: NonZeroU8,
    rows: NonZeroU8,
}

impl GridGeometry {
    pub const SHARED_TAB: Self = Self::new(10, 10);
    pub const CHARACTER_STASH: Self = Self::new(10, 10);
    pub const INVENTORY: Self = Self::new(10, 4);
    pub const CUBE: Self = Self::new(3, 4);

    const fn new(columns: u8, rows: u8) -> Self {
        Self {
            columns: NonZeroU8::new(columns).expect("a grid holds at least one column"),
            rows: NonZeroU8::new(rows).expect("a grid holds at least one row"),
        }
    }

    #[must_use]
    pub const fn columns(self) -> u8 {
        self.columns.get()
    }

    #[must_use]
    pub const fn rows(self) -> u8 {
        self.rows.get()
    }

    #[must_use]
    const fn holds(self, cell: GridPosition, footprint: Footprint) -> bool {
        cell.column.0 as u16 + footprint.width() as u16 <= self.columns.get() as u16
            && cell.row.0 as u16 + footprint.height() as u16 <= self.rows.get() as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridPosition {
    column: Column,
    row: Row,
}

impl GridPosition {
    pub fn new(column: Column, row: Row) -> Result<Self, ModelError> {
        GridCell::new(u32::from(column.0), u32::from(row.0))?;
        Ok(Self { column, row })
    }

    #[must_use]
    pub const fn from_cell(cell: GridCell) -> Self {
        #[expect(clippy::cast_possible_truncation, reason = "a grid cell holds 4 bits")]
        Self {
            column: Column(cell.column() as u8),
            row: Row(cell.row() as u8),
        }
    }

    #[must_use]
    pub fn to_cell(self) -> GridCell {
        GridCell::new(u32::from(self.column.0), u32::from(self.row.0))
            .expect("a grid position is built inside the cell address space")
    }

    #[must_use]
    pub const fn column(self) -> Column {
        self.column
    }

    #[must_use]
    pub const fn row(self) -> Row {
        self.row
    }
}

impl fmt::Display for GridPosition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "column {} row {}", self.column, self.row)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Occupancy([u16; ADDRESS_ROWS]);

impl Occupancy {
    fn mask(cell: GridPosition, footprint: Footprint) -> u16 {
        let span = (1u32 << u32::from(footprint.width())) - 1;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a grid row holds 16 columns"
        )]
        let mask = (span << u32::from(cell.column.0)) as u16;
        mask
    }

    const fn rows(cell: GridPosition, footprint: Footprint) -> std::ops::Range<usize> {
        let start = cell.row.0 as usize;
        start..start + footprint.height() as usize
    }

    fn is_free(&self, cell: GridPosition, footprint: Footprint) -> bool {
        let mask = Self::mask(cell, footprint);
        Self::rows(cell, footprint).all(|row| self.0[row] & mask == 0)
    }

    fn mark(&mut self, cell: GridPosition, footprint: Footprint) {
        let mask = Self::mask(cell, footprint);
        for row in Self::rows(cell, footprint) {
            self.0[row] |= mask;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    origin: GridPosition,
    object: Object,
}

impl Slot {
    #[must_use]
    pub const fn origin(&self) -> GridPosition {
        self.origin
    }

    #[must_use]
    pub const fn object(&self) -> &Object {
        &self.object
    }

    #[must_use]
    pub fn into_object(self) -> Object {
        self.object
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    geometry: GridGeometry,
    slots: Vec<Slot>,
}

impl Grid {
    #[must_use]
    pub const fn new(geometry: GridGeometry) -> Self {
        Self {
            geometry,
            slots: Vec::new(),
        }
    }

    #[must_use]
    pub const fn geometry(&self) -> GridGeometry {
        self.geometry
    }

    #[must_use]
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    #[must_use]
    pub fn into_slots(self) -> Vec<Slot> {
        self.slots
    }

    #[must_use]
    pub fn find(&self, fingerprint: Fingerprint) -> Option<&Object> {
        self.slots
            .iter()
            .find(|slot| slot.object.fingerprint() == fingerprint)
            .map(|slot| &slot.object)
    }

    pub fn take(&mut self, fingerprint: Fingerprint) -> Option<Object> {
        let index = self
            .slots
            .iter()
            .position(|slot| slot.object.fingerprint() == fingerprint)?;
        Some(self.slots.remove(index).object)
    }

    pub fn place(
        &mut self,
        id: &GridId,
        object: Object,
        at: GridPosition,
    ) -> Result<(), RuleViolation> {
        let footprint = object.footprint();
        if !self.geometry.holds(at, footprint) {
            return Err(RuleViolation::OutsideGrid {
                container: id.clone(),
                cell: at,
                footprint,
                geometry: self.geometry,
            });
        }
        if !self.occupancy().is_free(at, footprint) {
            return Err(RuleViolation::CellTaken {
                container: id.clone(),
                cell: at,
            });
        }
        self.slots.push(Slot { origin: at, object });
        Ok(())
    }

    pub fn append(&mut self, id: &GridId, object: Object) -> Result<GridPosition, RuleViolation> {
        let footprint = object.footprint();
        let at = self
            .first_free(footprint)
            .ok_or_else(|| RuleViolation::GridFull {
                container: id.clone(),
                footprint,
            })?;
        self.slots.push(Slot { origin: at, object });
        Ok(at)
    }

    #[must_use]
    pub fn first_free(&self, footprint: Footprint) -> Option<GridPosition> {
        if footprint.width() > self.geometry.columns() || footprint.height() > self.geometry.rows()
        {
            return None;
        }
        let occupancy = self.occupancy();
        let last_column = self.geometry.columns() - footprint.width();
        let last_row = self.geometry.rows() - footprint.height();
        for row in 0..=last_row {
            for column in 0..=last_column {
                let cell = GridPosition {
                    column: Column(column),
                    row: Row(row),
                };
                if occupancy.is_free(cell, footprint) {
                    return Some(cell);
                }
            }
        }
        None
    }

    fn occupancy(&self) -> Occupancy {
        let mut occupancy = Occupancy::default();
        for slot in &self.slots {
            occupancy.mark(slot.origin, slot.object.footprint());
        }
        occupancy
    }
}
