use std::fmt;
use std::num::NonZeroU8;

use crate::error::{PileError, ValueError};
use crate::object::{Fingerprint, Object};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PileCount(u8);

impl PileCount {
    pub const MAX: u8 = 99;
    pub const ZERO: Self = Self(0);

    pub const fn new(count: u8) -> Result<Self, ValueError> {
        if count > Self::MAX {
            return Err(ValueError::PileCountOverCap {
                count: count as u16,
                max: Self::MAX,
            });
        }
        Ok(Self(count))
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    #[must_use]
    const fn room(self) -> u8 {
        Self::MAX - self.0
    }
}

impl fmt::Display for PileCount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PileUnits(NonZeroU8);

impl PileUnits {
    pub const ONE: Self = Self(NonZeroU8::MIN);

    pub const fn new(units: u8) -> Result<Self, ValueError> {
        match NonZeroU8::new(units) {
            Some(units) if units.get() <= PileCount::MAX => Ok(Self(units)),
            Some(units) => Err(ValueError::PileCountOverCap {
                count: units.get() as u16,
                max: PileCount::MAX,
            }),
            None => Err(ValueError::PileCountOverCap {
                count: 0,
                max: PileCount::MAX,
            }),
        }
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0.get()
    }
}

impl fmt::Display for PileUnits {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PileEntry {
    object: Object,
    count: PileCount,
}

impl PileEntry {
    #[must_use]
    pub const fn object(&self) -> &Object {
        &self.object
    }

    #[must_use]
    pub const fn count(&self) -> PileCount {
        self.count
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pile {
    entries: Vec<PileEntry>,
}

impl Pile {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[PileEntry] {
        &self.entries
    }

    #[must_use]
    pub fn units(&self, fingerprint: Fingerprint) -> u16 {
        self.entries
            .iter()
            .filter(|entry| entry.object.fingerprint() == fingerprint)
            .map(|entry| u16::from(entry.count.get()))
            .sum()
    }

    #[must_use]
    pub fn find(&self, fingerprint: Fingerprint) -> Option<&Object> {
        self.entries
            .iter()
            .find(|entry| entry.object.fingerprint() == fingerprint)
            .map(|entry| &entry.object)
    }

    pub fn restore(&mut self, object: Object, count: PileCount) {
        self.entries.push(PileEntry { object, count });
    }

    pub fn add(&mut self, object: Object, units: PileUnits) {
        let fingerprint = object.fingerprint();
        let mut left = units.get();
        for entry in &mut self.entries {
            if left == 0 {
                break;
            }
            if entry.object.fingerprint() != fingerprint {
                continue;
            }
            let stacked = entry.count.room().min(left);
            entry.count = PileCount(entry.count.get() + stacked);
            left -= stacked;
        }
        if left > 0 {
            self.entries.push(PileEntry {
                object,
                count: PileCount(left),
            });
        }
    }

    pub fn take(
        &mut self,
        fingerprint: Fingerprint,
        units: PileUnits,
    ) -> Result<Object, PileError> {
        let held = self.units(fingerprint);
        if held < u16::from(units.get()) {
            return Err(PileError::NotEnoughUnits {
                fingerprint,
                held,
                asked: units,
            });
        }
        let mut left = units.get();
        let mut taken = None;
        let mut emptied = Vec::new();
        for (index, entry) in self.entries.iter_mut().enumerate() {
            if entry.object.fingerprint() != fingerprint {
                continue;
            }
            if taken.is_none() {
                taken = Some(entry.object.clone());
            }
            if left == 0 {
                break;
            }
            let removed = entry.count.get().min(left);
            entry.count = PileCount(entry.count.get() - removed);
            left -= removed;
            if entry.count.get() == 0 {
                emptied.push(index);
            }
        }
        for index in emptied.into_iter().rev() {
            self.entries.remove(index);
        }
        taken.ok_or(PileError::NotEnoughUnits {
            fingerprint,
            held,
            asked: units,
        })
    }
}
